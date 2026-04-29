// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPServer — serveur HTTP multi-connexions
//
// Fonctions exportées (convention C) :
//
//   HTTPServer_init(self_ptr)                         → void  constructeur
//   HTTPServer_set_port(self_ptr, port)               → void
//   HTTPServer_set_host(self_ptr, host_ptr)           → void
//   HTTPServer_set_workers(self_ptr, n)               → void  threads accepteurs
//   HTTPServer_set_root_path(self_ptr, path_ptr)      → void  répertoire fichiers statiques
//   HTTPServer_route(self_ptr, path, method, fat_ptr) → void  enregistre une route
//   HTTPServer_run(self_ptr)                          → void  démarre (bloquant)
//
// Fonctions statiques (appelées depuis un handler) :
//
//   HTTPServer_req_path(req)           → i64  chemin (sans query string)
//   HTTPServer_req_method(req)         → i64  méthode HTTP en majuscules
//   HTTPServer_req_body(req)           → i64  corps de la requête
//   HTTPServer_req_header(req, name)   → i64  valeur d'un en-tête (vide si absent)
//   HTTPServer_req_query(req, key)     → i64  valeur d'un paramètre query string
//   HTTPServer_respond(req, status, body) → void  envoie la réponse
//   HTTPServer_set_resp_header(req, name, value) → void  ajoute un en-tête à la réponse
//
// Architecture multi-thread :
//   Le serveur écoute sur `host:port`. `workers` threads appellent chacun
//   `server.recv()` en boucle (modèle "accept pool" recommandé par tiny_http).
//   Chaque requête est traitée dans le thread qui l'a reçue.
//
// Convention handler Ocara :
//   Le handler est une closure Ocara `nameless(req:int): int { … }`.
//   La signature compilée est : extern "C" fn(env_ptr: i64, req: i64) -> i64
//   req est un pointeur vers un OcaraHttpContext alloué par le serveur.
//
// Note sécurité concurrente :
//   Les captures partagées entre handlers (heap_promoted) ne sont pas
//   protégées par un mutex. Des accès concurrents constituent un data race.
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    collections::HashMap,
    sync::Arc,
};

use crate::{alloc_str, ptr_to_str};

// Macro safe pour les logs serveur (contourne le write(2) shadowé)
macro_rules! server_log {
    ($($arg:tt)*) => {
        crate::write_stderr_raw(format!($($arg)*).as_bytes())
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Types handler
// ─────────────────────────────────────────────────────────────────────────────

/// Signature d'un handler Ocara : fn(env_ptr, req_handle) → i64
type OcaraHandlerFn = unsafe extern "C" fn(i64, i64) -> i64;

/// Wrapper Send pour les raw pointers de closure.
/// Safety : les closures Ocara sont allouées sur le tas (heap_promoted) et
/// vivent aussi longtemps que le serveur tourne. L'appelant est responsable
/// de la synchronisation des accès concurrents aux données partagées.
#[derive(Clone)]
struct SendHandler {
    func_ptr: i64,
    env_ptr:  i64,
}
unsafe impl Send for SendHandler {}
unsafe impl Sync for SendHandler {}

// ─────────────────────────────────────────────────────────────────────────────
// Route
// ─────────────────────────────────────────────────────────────────────────────

struct Route {
    path:    String,
    method:  String,   // en majuscules
    handler: SendHandler,
}

// ─────────────────────────────────────────────────────────────────────────────
// OcaraHttpServer — struct interne
// ─────────────────────────────────────────────────────────────────────────────

struct OcaraHttpServer {
    port:          u16,
    host:          String,
    workers:       usize,
    routes:        Vec<Route>,
    root_path:     Option<String>,  // Répertoire racine pour fichiers statiques
    error_handlers: HashMap<u16, SendHandler>, // Handlers pour codes d'erreur (404, 500, etc.)
}

impl OcaraHttpServer {
    fn new() -> Self {
        OcaraHttpServer {
            port:           8080,
            host:           "0.0.0.0".into(),
            workers:        4,
            routes:         Vec::new(),
            root_path:      None,
            error_handlers: HashMap::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OcaraHttpContext — contexte par requête
// ─────────────────────────────────────────────────────────────────────────────

struct OcaraHttpContext {
    // Données de la requête (lues une fois, mises en cache)
    path:    String,
    method:  String,
    body:    String,
    headers: HashMap<String, String>,
    query:   HashMap<String, String>,
    // Construction de la réponse
    resp_status:  u16,
    resp_body:    String,
    resp_headers: Vec<tiny_http::Header>,
    // Requête tiny_http (consommée lors de l'envoi de la réponse)
    request: Option<tiny_http::Request>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Accès aux structs via pointeurs opaques
// ─────────────────────────────────────────────────────────────────────────────

#[inline]
unsafe fn server_from_slot(self_ptr: i64) -> &'static mut OcaraHttpServer {
    let slot  = self_ptr as *const i64;
    let inner = *slot as *mut OcaraHttpServer;
    &mut *inner
}

#[inline]
unsafe fn ctx_ref(req: i64) -> &'static mut OcaraHttpContext {
    &mut *(req as *mut OcaraHttpContext)
}

// ─────────────────────────────────────────────────────────────────────────────
// Parsing de la query string
// ─────────────────────────────────────────────────────────────────────────────

fn parse_query(raw: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in raw.split('&') {
        let mut it = pair.splitn(2, '=');
        let key = it.next().unwrap_or("").to_string();
        let val = it.next().unwrap_or("").to_string();
        if !key.is_empty() {
            // Décodage URL simple (+ → espace, %XX → caractère)
            map.insert(url_decode(&key), url_decode(&val));
        }
    }
    map
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => { out.push(' '); i += 1; }
            b'%' if i + 2 < bytes.len() => {
                if let Ok(hex) = std::str::from_utf8(&bytes[i+1..i+3]) {
                    if let Ok(n) = u8::from_str_radix(hex, 16) {
                        out.push(n as char);
                        i += 3;
                        continue;
                    }
                }
                out.push(bytes[i] as char);
                i += 1;
            }
            b => { out.push(b as char); i += 1; }
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Serveur de fichiers statiques
// ─────────────────────────────────────────────────────────────────────────────

/// Détermine le MIME type selon l'extension du fichier.
fn mime_type_from_path(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css"  => "text/css; charset=utf-8",
        "js"   => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "xml"  => "application/xml; charset=utf-8",
        "txt"  => "text/plain; charset=utf-8",
        "md"   => "text/markdown; charset=utf-8",
        "png"  => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif"  => "image/gif",
        "svg"  => "image/svg+xml",
        "webp" => "image/webp",
        "ico"  => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf"  => "font/ttf",
        "eot"  => "application/vnd.ms-fontobject",
        "pdf"  => "application/pdf",
        "zip"  => "application/zip",
        _      => "application/octet-stream",
    }
}

/// Tente de servir un fichier statique depuis le répertoire root_path.
/// Retourne true si un fichier a été servi, false sinon.
fn try_serve_static_file(req_handle: i64, req_path: &str, root_path: Option<&str>) -> bool {
    let root = match root_path {
        Some(r) => r,
        None => return false,
    };

    // Nettoyer le chemin : retirer le / initial et décoder
    let clean_path = req_path.trim_start_matches('/');
    
    // Protection contre path traversal : bloquer ../ ou ../
    if clean_path.contains("..") {
        return false;
    }

    // Construire le chemin complet
    let file_path = std::path::Path::new(root).join(clean_path);
    
    // Vérifier que le fichier canonique reste dans le root (double protection)
    if let Ok(canonical) = file_path.canonicalize() {
        if let Ok(root_canonical) = std::path::Path::new(root).canonicalize() {
            if !canonical.starts_with(&root_canonical) {
                return false;
            }
        }
    }

    // Tenter de lire le fichier
    let content = match std::fs::read(&file_path) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };

    // Déterminer le MIME type
    let mime = mime_type_from_path(clean_path);

    // Remplir la réponse
    let ctx = unsafe { ctx_ref(req_handle) };
    ctx.resp_status = 200;
    ctx.resp_body = String::from_utf8_lossy(&content).to_string();
    
    // Ajouter Content-Type
    let header = tiny_http::Header::from_bytes(
        &b"Content-Type"[..],
        mime.as_bytes(),
    ).unwrap();
    ctx.resp_headers.push(header);

    true
}

// ─────────────────────────────────────────────────────────────────────────────
// Traitement d'une requête
// ─────────────────────────────────────────────────────────────────────────────

fn handle_request(
    mut request: tiny_http::Request, 
    routes: &[Route], 
    root_path: Option<&str>,
    error_handlers: &HashMap<u16, SendHandler>
) {
    // Lire le corps
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);

    // Décomposer l'URL en chemin + query string
    let full_url = request.url().to_string();
    let (path, query_str) = match full_url.find('?') {
        Some(pos) => (&full_url[..pos], &full_url[pos + 1..]),
        None      => (full_url.as_str(), ""),
    };

    let method = request.method().to_string().to_uppercase();

    // Collecter les en-têtes de la requête
    let mut headers: HashMap<String, String> = HashMap::new();
    for h in request.headers() {
        headers.insert(
            h.field.to_string().to_lowercase(),
            h.value.to_string(),
        );
    }

    // Chercher une route correspondante
    let handler = routes.iter().find(|r| {
        r.method == method && (r.path == path || r.path == "*")
    }).map(|r| r.handler.clone());

    // Construire le contexte de requête
    let path_str   = path.to_string();
    let method_str = method.clone();
    let ctx = Box::new(OcaraHttpContext {
        path:         path_str.clone(),
        method:       method_str.clone(),
        body,
        headers,
        query:        parse_query(query_str),
        resp_status:  200,
        resp_body:    String::new(),
        resp_headers: Vec::new(),
        request:      Some(request),
    });
    let req_handle = Box::into_raw(ctx) as i64;

    // Appeler le handler ou tenter de servir un fichier statique
    if let Some(h) = handler {
        let f: OcaraHandlerFn = unsafe { std::mem::transmute(h.func_ptr as usize) };
        unsafe { f(h.env_ptr, req_handle) };
    } else if method_str == "GET" {
        // Si GET / sans route → essayer /index.html automatiquement
        let serve_path = if path_str == "/" { "/index.html" } else { &path_str };
        
        if try_serve_static_file(req_handle, serve_path, root_path) {
            // Fichier statique servi avec succès
        } else {
            // Aucune route trouvée et pas de fichier statique → 404
            let ctx = unsafe { ctx_ref(req_handle) };
            ctx.resp_status = 404;
            
            // Chercher un handler d'erreur 404 personnalisé
            if let Some(error_h) = error_handlers.get(&404) {
                let f: OcaraHandlerFn = unsafe { std::mem::transmute(error_h.func_ptr as usize) };
                unsafe { f(error_h.env_ptr, req_handle) };
            } else {
                ctx.resp_body = format!("404 Not Found: {} {}", method_str, path_str);
            }
        }
    } else {
        // Méthode non-GET sans route → 404
        let ctx = unsafe { ctx_ref(req_handle) };
        ctx.resp_status = 404;
        
        // Chercher un handler d'erreur 404 personnalisé
        if let Some(error_h) = error_handlers.get(&404) {
            let f: OcaraHandlerFn = unsafe { std::mem::transmute(error_h.func_ptr as usize) };
            unsafe { f(error_h.env_ptr, req_handle) };
        } else {
            ctx.resp_body = format!("404 Not Found: {} {}", method_str, path_str);
        }
    }

    // Envoyer la réponse
    let ctx = unsafe { &mut *(req_handle as *mut OcaraHttpContext) };
    
    // Ajouter Content-Type par défaut si non défini
    let has_content_type = ctx.resp_headers.iter().any(|h| {
        h.field.to_string().eq_ignore_ascii_case("Content-Type")
    });
    if !has_content_type {
        let default_ct = tiny_http::Header::from_bytes(
            &b"Content-Type"[..],
            &b"text/html; charset=utf-8"[..],
        ).unwrap();
        ctx.resp_headers.push(default_ct);
    }
    
    if let Some(req) = ctx.request.take() {
        let status = tiny_http::StatusCode(ctx.resp_status);
        let mut resp = tiny_http::Response::from_string(ctx.resp_body.clone())
            .with_status_code(status);
        // Ajouter les en-têtes de réponse
        for h in ctx.resp_headers.drain(..) {
            resp = resp.with_header(h);
        }
        let _ = req.respond(resp);
    }

    // Libérer le contexte
    drop(unsafe { Box::from_raw(req_handle as *mut OcaraHttpContext) });
}

// ─────────────────────────────────────────────────────────────────────────────
// API publique exportée (convention C)
// ─────────────────────────────────────────────────────────────────────────────

/// Constructeur : alloue un OcaraHttpServer et écrit le pointeur dans le slot.
#[no_mangle]
pub extern "C" fn HTTPServer_init(self_ptr: i64) {
    let s   = Box::new(OcaraHttpServer::new());
    let raw = Box::into_raw(s) as i64;
    unsafe { *(self_ptr as *mut i64) = raw; }
}

/// Définit le port d'écoute (défaut : 8080).
#[no_mangle]
pub extern "C" fn HTTPServer_set_port(self_ptr: i64, port: i64) {
    let s = unsafe { server_from_slot(self_ptr) };
    s.port = port as u16;
}

/// Définit l'adresse d'écoute (défaut : "0.0.0.0").
#[no_mangle]
pub extern "C" fn HTTPServer_set_host(self_ptr: i64, host_ptr: i64) {
    let s = unsafe { server_from_slot(self_ptr) };
    s.host = unsafe { ptr_to_str(host_ptr).to_string() };
}

/// Définit le nombre de threads workers (défaut : 4).
#[no_mangle]
pub extern "C" fn HTTPServer_set_workers(self_ptr: i64, n: i64) {
    let s = unsafe { server_from_slot(self_ptr) };
    s.workers = n.max(1) as usize;
}

/// Définit le répertoire racine pour servir les fichiers statiques.
/// Si défini, les requêtes qui ne matchent aucune route tenteront de servir
/// un fichier depuis ce répertoire. Protégé contre path traversal.
#[no_mangle]
pub extern "C" fn HTTPServer_set_root_path(self_ptr: i64, path_ptr: i64) {
    let s = unsafe { server_from_slot(self_ptr) };
    let path = unsafe { ptr_to_str(path_ptr).to_string() };
    s.root_path = if path.is_empty() { None } else { Some(path) };
}

/// Enregistre une route.
/// `fat_ptr` pointe sur un struct {func_ptr: i64, env_ptr: i64} (fat pointer Ocara).
#[no_mangle]
pub extern "C" fn HTTPServer_route(
    self_ptr: i64,
    path_ptr: i64,
    method_ptr: i64,
    fat_ptr: i64,
) {
    let s          = unsafe { server_from_slot(self_ptr) };
    let func_ptr   = unsafe { *(fat_ptr as *const i64) };
    let env_ptr    = unsafe { *((fat_ptr as *const i64).add(1)) };
    let path   = unsafe { ptr_to_str(path_ptr).to_string() };
    let method = unsafe { ptr_to_str(method_ptr).to_string().to_uppercase() };
    s.routes.push(Route {
        path,
        method,
        handler: SendHandler { func_ptr, env_ptr },
    });
}

/// Enregistre un handler pour un code d'erreur HTTP spécifique.
/// Permet de personnaliser les pages d'erreur (404, 500, etc.).
#[no_mangle]
pub extern "C" fn HTTPServer_route_error(
    self_ptr: i64,
    code: i64,
    fat_ptr: i64,
) {
    let s        = unsafe { server_from_slot(self_ptr) };
    let func_ptr = unsafe { *(fat_ptr as *const i64) };
    let env_ptr  = unsafe { *((fat_ptr as *const i64).add(1)) };
    s.error_handlers.insert(code as u16, SendHandler { func_ptr, env_ptr });
}

/// Démarre le serveur (appel bloquant).
/// Lance `workers` threads qui acceptent les connexions en parallèle.
#[no_mangle]
pub extern "C" fn HTTPServer_run(self_ptr: i64) {
    let data   = unsafe { server_from_slot(self_ptr) };
    let addr   = format!("{}:{}", data.host, data.port);
    let server = match tiny_http::Server::http(&addr) {
        Ok(s)  => Arc::new(s),
        Err(e) => {
            server_log!("HTTPServer: unable to start on {} : {}\n", addr, e);
            return;
        }
    };
    let routes: Arc<Vec<Route>> = Arc::new(std::mem::take(&mut data.routes));
    let root_path: Arc<Option<String>> = Arc::new(data.root_path.clone());
    let error_handlers: Arc<HashMap<u16, SendHandler>> = Arc::new(std::mem::take(&mut data.error_handlers));

    server_log!("HTTPServer: listening on http://{}\n", addr);

    let handles: Vec<_> = (0..data.workers).map(|_| {
        let server = Arc::clone(&server);
        let routes = Arc::clone(&routes);
        let root_path = Arc::clone(&root_path);
        let error_handlers = Arc::clone(&error_handlers);
        std::thread::spawn(move || {
            loop {
                match server.recv() {
                    Ok(request) => handle_request(request, &routes, root_path.as_deref(), &error_handlers),
                    Err(_)      => break,
                }
            }
        })
    }).collect();

    for h in handles {
        let _ = h.join();
    }
}

// ─── Méthodes statiques — lecture de la requête (appelées depuis un handler) ─

/// Retourne le chemin de la requête courante (sans query string).
#[no_mangle]
pub extern "C" fn HTTPServer_req_path(req: i64) -> i64 {
    let path = unsafe { ctx_ref(req).path.clone() };
    unsafe { alloc_str(&path) }
}

/// Retourne la méthode HTTP de la requête courante (ex: "GET").
#[no_mangle]
pub extern "C" fn HTTPServer_req_method(req: i64) -> i64 {
    let method = unsafe { ctx_ref(req).method.clone() };
    unsafe { alloc_str(&method) }
}

/// Retourne le corps de la requête courante.
#[no_mangle]
pub extern "C" fn HTTPServer_req_body(req: i64) -> i64 {
    let body = unsafe { ctx_ref(req).body.clone() };
    unsafe { alloc_str(&body) }
}

/// Retourne la valeur d'un en-tête de la requête (clé insensible à la casse).
/// Retourne une chaîne vide si l'en-tête est absent.
#[no_mangle]
pub extern "C" fn HTTPServer_req_header(req: i64, name_ptr: i64) -> i64 {
    let name = unsafe { ptr_to_str(name_ptr).to_lowercase() };
    let val  = unsafe { ctx_ref(req) }.headers.get(&name)
        .cloned()
        .unwrap_or_default();
    unsafe { alloc_str(&val) }
}

/// Retourne la valeur d'un paramètre query string.
/// Retourne une chaîne vide si le paramètre est absent.
#[no_mangle]
pub extern "C" fn HTTPServer_req_query(req: i64, key_ptr: i64) -> i64 {
    let key = unsafe { ptr_to_str(key_ptr).to_string() };
    let val = unsafe { ctx_ref(req) }.query.get(&key)
        .cloned()
        .unwrap_or_default();
    unsafe { alloc_str(&val) }
}

// ─── Méthodes statiques — construction de la réponse ─────────────────────────

/// Définit le statut HTTP et le corps de la réponse.
/// Peut être appelé plusieurs fois : seul le dernier appel est utilisé.
#[no_mangle]
pub extern "C" fn HTTPServer_respond(req: i64, status: i64, body_ptr: i64) {
    let body      = unsafe { ptr_to_str(body_ptr).to_string() };
    let ctx       = unsafe { ctx_ref(req) };
    ctx.resp_status = status as u16;
    ctx.resp_body   = body;
}

/// Ajoute un en-tête à la réponse (ex: "Content-Type", "text/html").
#[no_mangle]
pub extern "C" fn HTTPServer_set_resp_header(req: i64, name_ptr: i64, value_ptr: i64) {
    let name  = unsafe { ptr_to_str(name_ptr).to_string() };
    let value = unsafe { ptr_to_str(value_ptr).to_string() };
    let header_str = format!("{}: {}", name, value);
    let ctx = unsafe { ctx_ref(req) };
    if let Ok(h) = header_str.parse::<tiny_http::Header>() {
        ctx.resp_headers.push(h);
    }
}
