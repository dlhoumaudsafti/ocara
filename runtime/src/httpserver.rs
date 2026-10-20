// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPServer / ocara.HTTPServerRequest — serveur HTTP multi-connexions
//
// Fonctions exportées pour `HTTPServer` (convention C) :
//
//   HTTPServer_init(self_ptr)                         → void  constructeur
//   HTTPServer_port(self_ptr, port)               → void
//   HTTPServer_host(self_ptr, host_ptr)           → void
//   HTTPServer_workers(self_ptr, n)               → void  threads accepteurs
//   HTTPServer_rootPath(self_ptr, path_ptr)      → void  répertoire fichiers statiques
//   HTTPServer_route(self_ptr, path, method, fat_ptr) → void  enregistre une route
//   HTTPServer_routeError(self_ptr, code, fat_ptr) → void  enregistre un handler d'erreur
//   HTTPServer_run(self_ptr)                          → void  démarre (bloquant)
//
// Fonctions exportées pour `HTTPServerRequest` (méthodes d'instance, appelées
// depuis un handler — voir docs/roadmap.d/stdlib-httpserver-request-object.md,
// désormais clos) :
//
//   HTTPServerRequest_path(req)           → i64  chemin (sans query string)
//   HTTPServerRequest_method(req)         → i64  méthode HTTP en majuscules
//   HTTPServerRequest_body(req)           → i64  corps brut de la requête
//   HTTPServerRequest_header(req, name)   → i64  valeur d'un en-tête (comparaison INSENSIBLE
//                                                 à la casse ; vide si absent)
//   HTTPServerRequest_headers(req)        → i64  map<string,string|int|float|bool|null> — TOUS
//                                                 les en-têtes, clés dans leur casse d'ORIGINE
//                                                 (seule la recherche de header() est insensible
//                                                 à la casse, pas les clés de cette map)
//   HTTPServerRequest_query(req, key)     → i64  valeur d'un paramètre query string
//   HTTPServerRequest_param_1(req, key)         → i64 (mixed)  voir param() ci-dessous, method=null
//   HTTPServerRequest_param(req, key, method)   → i64 (mixed)  accessoir universel — voir
//                                                 `build_params_buckets`/`lookup_param` pour la
//                                                 règle de précédence GET/body exacte
//   HTTPServerRequest_params(req)         → i64  map<string, map<string,mixed>> — 10 clés
//                                                 (CONNECT/DELETE/GET/HEAD/OPTIONS/PATCH/POST/
//                                                 PUT/QUERY/TRACE), toujours présentes
//   HTTPServerRequest_respond(req, status, body) → void  envoie la réponse
//   HTTPServerRequest_respondHeader(req, name, value) → void  ajoute un en-tête à la réponse
//
// Parsing du corps (voir docs/roadmap.d/stdlib-httpserver-post-body-parsing.md,
// désormais clos, et le nouveau support multipart) : `application/x-www-form-
// urlencoded` réutilise `parse_query`/`form_decode` (déjà utilisées pour la query
// string) contre le corps ; `multipart/form-data` est parsé par un mini-parseur
// écrit à la main (`parse_multipart`) — voir sa doc pour les choix de tolérance
// (CRLF vs LF) et de représentation des champs fichier.
//
// Architecture multi-thread :
//   Le serveur écoute sur `host:port`. `workers` threads appellent chacun
//   `server.recv()` en boucle (modèle "accept pool" recommandé par tiny_http).
//   Chaque requête est traitée dans le thread qui l'a reçue.
//
// Convention handler Ocara :
//   Le handler est une closure Ocara `nameless(req:HTTPServerRequest): int { … }`.
//   La signature compilée est : extern "C" fn(env_ptr: i64, req: i64) -> i64
//   req est un pointeur vers un OcaraHttpContext alloué par le serveur, déguisé
//   en `HTTPServerRequest` côté Ocara (voir src/builtins/httpserver.rs,
//   `request_class()`) — AUCUN changement d'ABI par rapport à l'ancien `req:int` :
//   un paramètre `Type::Named(classe)` se compile déjà comme un seul i64 (même
//   mécanisme que `nameless(db:SQLite): void`, voir src/lower/expr.d/nameless.rs).
//
// Note sécurité concurrente :
//   L'invocation d'un handler (route ou erreur) est sérialisée par un mutex
//   global, commun à tous les serveurs du programme (`HANDLER_LOCK`) — un
//   verrou par serveur laissait deux serveurs partageant une capture se
//   marcher dessus (incrémentations perdues) :
//   deux handlers Ocara ne s'exécutent JAMAIS en même temps, ce qui élimine
//   par construction le data race sur les captures partagées (heap_promoted)
//   entre deux exécutions concurrentes du même handler. Voir
//   docs/roadmap.d/runtime-httpserver-race-condition.md pour la justification
//   complète (option retenue face à un mutex générique sur `heap_promoted`,
//   qui pénaliserait aussi `Thread::spawn` et toute closure jamais partagée).
//   Coût assumé : la logique métier du handler n'est plus parallèle entre
//   requêtes (avant/après cet appel — lecture de la requête, envoi de la
//   réponse — reste parallèle). Changement de philosophie assumé par rapport
//   à `Thread`, qui reste "rapide par défaut, sûr sur demande (Mutex)".
//
// Gestion d'erreurs : HTTPServer_run() lève HTTPServerException en cas d'erreur de démarrage.
//
// Codes d'erreur HTTPServerException :
//   101 - SERVER_START    : Erreur de démarrage du serveur (binding)
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}},
    time::{Duration, Instant},
};

use crate::{alloc_str, ptr_to_str};
use crate::exception::throw_httpserver_exception;
use crate::httpserver_wire::{
    insert_request_header, is_valid_header_name, is_valid_header_value, parse_query, percent_decode, split_target,
};

// Codes d'erreur HTTPServerException
const ERR_SERVER_START: i64 = 101;
const ERR_INVALID_HEADER: i64 = 103;

/// Délai d'inactivité d'une connexion (lecture ou écriture) : un client qui
/// n'envoie plus rien est déconnecté (408) au lieu de bloquer un thread.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
/// Durée maximale de réception d'un corps de requête, quel que soit son débit.
const BODY_DEADLINE: Duration = Duration::from_secs(60);
/// Corps en cours de réception simultanément (hors workers) : au-delà, 503.
const MAX_RECEIVING_BODIES: usize = 256;

// Macro safe pour les logs serveur (contourne le write(2) shadowé)
macro_rules! server_log {
    ($($arg:tt)*) => {
        crate::write_stderr_raw(format!($($arg)*).as_bytes())
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Types handler
// ─────────────────────────────────────────────────────────────────────────────

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
    // Compilé UNE SEULE FOIS à l'enregistrement (`HTTPServer_route`), jamais
    // reparsé par requête — voir `parse_route_pattern`/`match_route`.
    pattern: RoutePattern,
    method:  String,   // en majuscules
    handler: SendHandler,
}

// ─────────────────────────────────────────────────────────────────────────────
// OcaraHttpServer — struct interne
// ─────────────────────────────────────────────────────────────────────────────

/// Taille maximale par défaut d'un corps de requête : 10 Mio.
const DEFAULT_MAX_BODY: usize = 10 * 1024 * 1024;

struct OcaraHttpServer {
    port:          u16,
    host:          String,
    workers:       usize,
    /// Taille maximale du corps de requête en octets (0 : sans limite) —
    /// au-delà, réponse 413 sans appeler le handler.
    max_body:      usize,
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
            max_body:       DEFAULT_MAX_BODY,
            routes:         Vec::new(),
            root_path:      None,
            error_handlers: HashMap::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OcaraHttpContext — contexte par requête
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) struct OcaraHttpContext {
    // Données de la requête (lues une fois, mises en cache)
    path:    String,
    method:  String,
    body:    String,
    // Casse D'ORIGINE (wire) préservée — tiny_http ne normalise jamais le nom
    // d'un champ d'en-tête (vérifié : `h.field.to_string()` reflète exactement
    // ce que le client a envoyé). Avant ce correctif, ces clés étaient
    // baissées en minuscules AU STOCKAGE (`.to_lowercase()` dans
    // `handle_request`) — suffisant pour `header(name)` (recherche déjà
    // insensible à la casse des deux côtés), mais aurait perdu la casse
    // d'origine pour `headers()` (nouvelle méthode, doit refléter la casse
    // TELLE QUE REÇUE). La recherche insensible à la casse se fait maintenant
    // au moment du LOOKUP (voir `header_lookup`), plus au stockage.
    pub(crate) headers: HashMap<String, String>,
    query:   HashMap<String, String>,
    // Paramètres GET (query string, toujours peuplé)/body (urlencoded ou
    // multipart, selon Content-Type) précalculés une seule fois à la
    // construction du contexte — voir `build_params_buckets`. 10 buckets
    // toujours présents (CONNECT/DELETE/GET/HEAD/OPTIONS/PATCH/POST/PUT/
    // QUERY/TRACE), vides sauf "GET" (query string + éventuel corps si la
    // méthode réelle est GET) et le bucket de la méthode réelle (corps, si
    // Content-Type reconnu et corps non vide).
    param_buckets: HashMap<String, HashMap<String, ParamValue>>,
    // Construction de la réponse
    resp_status:  u16,
    resp_body:    Vec<u8>,
    pub(crate) resp_headers: Vec<tiny_http::Header>,
    // Session ouverte par `req.session()` (voir runtime/src/httpsession.rs)
    pub(crate) session_id: Option<String>,
    // Requête tiny_http (consommée lors de l'envoi de la réponse)
    request: Option<tiny_http::Request>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Paramètres (query string + corps urlencoded/multipart) — HTTPServerRequest
// ─────────────────────────────────────────────────────────────────────────────

/// Les 10 "buckets" toujours présents dans `params()` — liste EXACTE donnée
/// par la spécification (pas la liste complète des méthodes HTTP existantes :
/// ex. pas de `LINK`/`UNLINK`, jamais utilisées en pratique par un serveur
/// applicatif ; `QUERY` y figure bien qu'assez rare, conformément à la liste
/// demandée).
pub(crate) const METHOD_BUCKETS: [&str; 10] = [
    "CONNECT", "DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT", "QUERY", "TRACE",
];

/// Valeur d'un paramètre de requête (query string, champ urlencoded, champ
/// multipart, OU segment de chemin typé `<nom:type>` — voir
/// docs/roadmap.d/stdlib-httpserver-route-params.md) — un texte simple, un
/// fichier uploadé (multipart avec `filename`), ou une valeur DÉJÀ TYPÉE
/// (`Int`/`Float`/`Bool`) issue d'un paramètre de chemin déjà validé/parsé par
/// le routeur AVANT que le handler ne s'exécute (voir `parse_path_param_value`).
/// Convertie en valeur `mixed` Ocara par `param_value_to_mixed`/
/// `file_to_mixed_map` uniquement au moment de construire la structure Ocara
/// (map/valeur) demandée — jamais avant, pour ne matérialiser des allocations
/// Ocara (`alloc_str`, `__map_new`, `__box_*`...) que pour les buckets
/// réellement lus.
///
/// `Int`/`Float`/`Bool` existent UNIQUEMENT pour les paramètres de chemin —
/// une valeur de query string ou de corps (`urlencoded`/`multipart`) est
/// TOUJOURS `Text` (une valeur HTTP "classique" est toujours une chaîne sur
/// le fil ; seul un paramètre de chemin porte une annotation de type explicite
/// dans le pattern de route lui-même, `<id:int>`).
#[derive(Clone)]
pub(crate) enum ParamValue {
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    File {
        filename:     String,
        content_type: String,
        // Représentation BINAIRE-SÛRE du contenu — voir la doc de
        // `file_to_mixed_map` pour la justification (`array<int>`, pas
        // `string` : `ptr_to_str`/`alloc_str` ne garantissent PAS un
        // aller-retour fidèle pour des octets qui ne sont pas de l'UTF-8
        // valide, voir runtime/src/lib.rs::ptr_to_str, qui retombe
        // silencieusement sur `""` en cas d'échec de décodage UTF-8 — même
        // convention binaire-sûre déjà établie par `File::readBytes`/
        // `writeBytes`, voir src/builtins/file.rs).
        content: Vec<u8>,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Paramètres de chemin — `<nom:type>` (docs/roadmap.d/stdlib-httpserver-route-params.md)
// ─────────────────────────────────────────────────────────────────────────────

/// Type déclaré d'un segment de chemin paramétré (`<id:int>`). `Str` couvre
/// à la fois `<nom:string>` et tout type inconnu/non reconnu — dégrade
/// silencieusement vers une chaîne brute plutôt que de rejeter
/// l'enregistrement de la route (même philosophie de tolérance que
/// `percent_decode`/`parse_query` (httpserver_wire.rs) : jamais planter sur
/// une entrée mal formée).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PathParamType {
    Int,
    Float,
    Bool,
    Str,
}

/// Un segment d'un chemin de route, entre deux `/` — soit littéral (comparé
/// tel quel), soit un paramètre nommé et typé.
pub(crate) enum PathSegment {
    Literal(String),
    Param { name: String, ty: PathParamType },
}

/// Pattern de route compilé UNE SEULE FOIS à l'enregistrement
/// (`HTTPServer_route`) — jamais reparsé à chaque requête entrante (voir
/// `match_route`, appelée par `handle_request`). `Wildcard` préserve le
/// comportement historique de `path == "*"` (n'importe quel chemin, pour la
/// méthode déclarée) — un `"*"` littéral ne peut de toute façon jamais
/// apparaître comme un segment `<nom:type>` valide, aucune ambiguïté entre
/// les deux formes.
pub(crate) enum RoutePattern {
    Wildcard,
    Segments(Vec<PathSegment>),
}

/// Parse un segment de chemin (texte entre deux `/`, JAMAIS encore décodé
/// URL) tel qu'écrit dans `server.route(path, ...)` — `<nom:type>` devient un
/// paramètre, tout le reste (y compris un `<...>` malformé, ex. sans `:`)
/// reste un littéral comparé tel quel : ne matchera jamais qu'un segment de
/// requête identique caractère pour caractère, ce qui inclut trivialement
/// "jamais", sans jamais paniquer sur une route mal écrite.
fn parse_path_segment(seg: &str) -> PathSegment {
    if let Some(inner) = seg.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
        if let Some((name, ty_str)) = inner.split_once(':') {
            let ty = match ty_str {
                "int"   => PathParamType::Int,
                "float" => PathParamType::Float,
                "bool"  => PathParamType::Bool,
                _       => PathParamType::Str, // "string" ou type inconnu
            };
            return PathSegment::Param { name: name.to_string(), ty };
        }
    }
    PathSegment::Literal(seg.to_string())
}

/// Compile `path` (tel qu'écrit dans `server.route(path, ...)`) en
/// `RoutePattern` — appelée UNE SEULE FOIS, à l'enregistrement.
pub(crate) fn parse_route_pattern(path: &str) -> RoutePattern {
    if path == "*" {
        return RoutePattern::Wildcard;
    }
    RoutePattern::Segments(path.split('/').map(parse_path_segment).collect())
}

/// Parse la valeur BRUTE d'un segment de requête (`raw`, jamais encore
/// décodé URL) selon le type déclaré du paramètre — `None` si `raw` ne
/// correspond pas au type déclaré (ex. `<id:int>` contre le segment
/// `"abc"`) : signale à `match_route` que CETTE route ne matche pas du tout
/// (voir sa doc — la route suivante est essayée, jamais de valeur
/// bidon/zéro substituée).
///
/// `decoded` est le segment de requête déjà décodé (`percent_decode`, `+`
/// conservé) — pour tous les types : `/i/%31` correspond à `<id:int>`. Un
/// `<x:float>` refuse `nan` et `inf`.
fn parse_path_param_value(decoded: &str, ty: PathParamType) -> Option<ParamValue> {
    match ty {
        PathParamType::Int   => decoded.parse::<i64>().ok().map(ParamValue::Int),
        PathParamType::Float => decoded.parse::<f64>().ok().filter(|f| f.is_finite()).map(ParamValue::Float),
        PathParamType::Bool  => match decoded {
            "true"  => Some(ParamValue::Bool(true)),
            "false" => Some(ParamValue::Bool(false)),
            _       => None,
        },
        PathParamType::Str => Some(ParamValue::Text(decoded.to_string())),
    }
}

/// Tente de faire correspondre `request_path` (le chemin RÉEL d'une requête,
/// ex. `/voitures/1`) à `pattern` (compilé une fois à l'enregistrement) —
/// `None` si le nombre de segments diffère, si un segment littéral ne
/// correspond pas exactement, ou si un segment paramétré échoue à parser
/// selon son type déclaré (voir `parse_path_param_value`). `Some(params)`
/// sinon, `params` étant vide pour une route sans paramètre (non-régression :
/// une route 100% littérale se comporte exactement comme avant ce ticket) ou
/// pour `Wildcard`.
///
/// Aucune règle de priorité "route statique avant route dynamique" séparée :
/// un segment littéral `ajouter` ne matche simplement JAMAIS un pattern
/// `<id:int>` (ni `<id:string>` à une position DIFFÉRENTE dans
/// l'enregistrement) puisque ce sont deux ROUTES distinctes, chacune avec son
/// propre pattern — l'ordre d'enregistrement (voir `handle_request`, premier
/// match gagne) suffit.
pub(crate) fn match_route(pattern: &RoutePattern, request_path: &str) -> Option<HashMap<String, ParamValue>> {
    match pattern {
        RoutePattern::Wildcard => Some(HashMap::new()),
        RoutePattern::Segments(segments) => {
            let request_segments: Vec<&str> = request_path.split('/').collect();
            if request_segments.len() != segments.len() {
                return None;
            }
            let mut params = HashMap::new();
            for (seg, req_seg) in segments.iter().zip(request_segments.iter()) {
                // Segment décodé APRÈS le découpage sur `/` : `%2F` reste
                // dans son segment ; `+` n'y est jamais un espace.
                let decoded = percent_decode(req_seg);
                match seg {
                    PathSegment::Literal(lit) => {
                        if lit != req_seg && *lit != decoded {
                            return None;
                        }
                    }
                    PathSegment::Param { name, ty } => {
                        let value = parse_path_param_value(&decoded, *ty)?;
                        params.insert(name.clone(), value);
                    }
                }
            }
            Some(params)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Accès aux structs via pointeurs opaques
// ─────────────────────────────────────────────────────────────────────────────

#[inline]
unsafe fn server_from_slot(self_ptr: i64) -> &'static mut OcaraHttpServer {
    unsafe {
        let slot  = self_ptr as *const i64;
        let inner = *slot as *mut OcaraHttpServer;
        &mut *inner
    }
}

#[inline]
pub(crate) unsafe fn ctx_ref(req: i64) -> &'static mut OcaraHttpContext {
    unsafe {
        &mut *(req as *mut OcaraHttpContext)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// En-têtes — recherche insensible à la casse (header(name), voir sa doc)
// ─────────────────────────────────────────────────────────────────────────────

/// Recherche insensible à la casse dans les en-têtes stockés avec leur casse
/// D'ORIGINE (voir `OcaraHttpContext.headers`) — une simple itération avec
/// `eq_ignore_ascii_case` : le nombre d'en-têtes d'une requête HTTP réelle
/// (quelques dizaines au plus) rend un balayage linéaire largement suffisant,
/// pas besoin de maintenir un index parallèle en minuscules.
pub(crate) fn header_lookup<'a>(headers: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    headers.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn content_type_of(headers: &HashMap<String, String>) -> Option<&str> {
    header_lookup(headers, "Content-Type")
}

// ─────────────────────────────────────────────────────────────────────────────
// Corps de requête — application/x-www-form-urlencoded & multipart/form-data
// ─────────────────────────────────────────────────────────────────────────────

/// Extrait le `boundary` d'un en-tête `Content-Type: multipart/form-data;
/// boundary=...` — `None` si `content_type` n'est pas `multipart/form-data`
/// (comparaison insensible à la casse sur le type MIME lui-même, voir RFC
/// 2045 §5.1 : les types/sous-types MIME ne sont jamais sensibles à la casse,
/// contrairement à la VALEUR de `boundary`, jamais modifiée ici) ou si aucun
/// paramètre `boundary=` n'est présent. Gère les deux formes autorisées par
/// RFC 2046 §5.1.1 : `boundary=xyz` (nue) et `boundary="xyz"` (quotée,
/// nécessaire dès que la valeur contient des caractères hors du jeu "token"
/// HTTP, ex. des espaces — courant avec les boundaries générées par les
/// navigateurs, `----WebKitFormBoundary...`).
pub(crate) fn parse_boundary(content_type: &str) -> Option<String> {
    let mut parts = content_type.split(';');
    let mime = parts.next()?.trim();
    if !mime.eq_ignore_ascii_case("multipart/form-data") {
        return None;
    }
    for param in parts {
        let param = param.trim();
        if let Some(rest) = strip_ci_prefix(param, "boundary=") {
            let unquoted = rest.trim().trim_matches('"');
            if !unquoted.is_empty() {
                return Some(unquoted.to_string());
            }
        }
    }
    None
}

/// Comme `str::strip_prefix`, mais insensible à la casse sur `prefix` — les
/// noms d'en-têtes/paramètres HTTP ne sont jamais sensibles à la casse (RFC
/// 7230 §3.2), contrairement à certaines de leurs VALEURS (ex. `boundary=`
/// lui-même insensible, mais la valeur qui suit reste prise telle quelle).
fn strip_ci_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes()) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

/// Recherche la première occurrence de `needle` dans `haystack` — équivalent
/// minimal de `[u8]::windows().position()` (aucune bibliothèque de recherche
/// de sous-chaîne binaire n'est déjà une dépendance de ce runtime, voir la
/// doc de `parse_multipart` sur le choix d'écrire ce parseur à la main).
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Un "part" (champ) d'un corps `multipart/form-data`, avant conversion en
/// `ParamValue` — `filename.is_some()` distingue un champ fichier d'un champ
/// texte simple (voir RFC 7578 §4.2).
pub(crate) struct MultipartPart {
    pub(crate) name:         String,
    pub(crate) filename:     Option<String>,
    pub(crate) content_type: Option<String>,
    pub(crate) body:         Vec<u8>,
}

/// Parse un corps `multipart/form-data` en une liste de parts — mini-parseur
/// écrit à la main plutôt qu'une dépendance externe : ce projet n'a AUCUNE
/// crate de parsing multipart déjà en dépendance (`tiny_http`, `ureq`, `url`,
/// `regex`, `serde_json`, `rusqlite`, `mysql` — vérifié dans
/// `runtime/Cargo.toml`, aucune ne fait ça), et `parse_query`/`percent_decode`
/// (httpserver_wire.rs) établissent déjà la convention de ce fichier : écrire son propre
/// parsing HTTP-adjacent plutôt que d'ajouter une dépendance pour un format
/// aussi simple à parser soi-même.
///
/// Tolérance CRLF/LF : RFC 7578/2046 imposent CRLF avant chaque délimiteur
/// `--boundary`, mais cette fonction cherche le délimiteur BRUT n'importe où
/// dans le corps (`find_subslice`, sans exiger un CRLF particulier juste
/// avant), puis ne retire qu'UNE SEULE terminaison de ligne (`\r\n` ou `\n`)
/// en tête/queue du contenu capturé — accepte donc aussi bien un corps strict
/// CRLF qu'un corps LF-seul (client non conforme), même esprit de tolérance
/// que le reste de ce fichier (`percent_decode` ne rejette jamais un `%` mal
/// formé, `parse_query` ignore silencieusement une paire sans `=`).
///
/// Limitation documentée (voir docs/roadmap.d/stdlib-httpserver-request-object.md,
/// section "Ce qui a été tranché") : plusieurs parts portant le MÊME `name`
/// (ex. un champ `photos[]` soumis plusieurs fois) — la dernière écrase les
/// précédentes dans le bucket résultat (comportement `HashMap::insert`
/// standard), jamais de panique. Hors périmètre d'un premier passage.
pub(crate) fn parse_multipart(body: &[u8], boundary: &str) -> Vec<MultipartPart> {
    let delim = format!("--{}", boundary).into_bytes();
    let mut parts = Vec::new();

    let Some(first) = find_subslice(body, &delim) else { return parts; };
    let mut pos = first + delim.len();

    loop {
        // Corps immédiatement après CE délimiteur : soit `--` (délimiteur
        // final, RFC 2046 §5.1.1), soit le contenu du part jusqu'au PROCHAIN
        // délimiteur.
        if body[pos..].starts_with(b"--") {
            break;
        }
        let next_rel = find_subslice(&body[pos..], &delim);
        let seg_end = match next_rel {
            Some(off) => pos + off,
            None => body.len(), // corps mal terminé (pas de délimiteur final) — tolérant, pas une erreur
        };
        let mut segment = &body[pos..seg_end];
        // Une seule terminaison de ligne pelée en tête (après le délimiteur)
        // et en queue (avant le délimiteur suivant) — voir la doc ci-dessus.
        if let Some(s) = segment.strip_prefix(b"\r\n".as_slice()) { segment = s; }
        else if let Some(s) = segment.strip_prefix(b"\n".as_slice()) { segment = s; }
        if let Some(s) = segment.strip_suffix(b"\r\n".as_slice()) { segment = s; }
        else if let Some(s) = segment.strip_suffix(b"\n".as_slice()) { segment = s; }

        if let Some(part) = parse_one_multipart_part(segment) {
            parts.push(part);
        }

        match next_rel {
            Some(_) => pos = seg_end + delim.len(),
            None => break,
        }
    }

    parts
}

/// Parse un seul "part" (déjà délimité par `parse_multipart`) : sépare son
/// petit bloc d'en-têtes (`Content-Disposition:`/`Content-Type:` optionnel)
/// du contenu, sur la première ligne vide (`\r\n\r\n` ou `\n\n`, même
/// tolérance CRLF/LF que `parse_multipart`). `None` si aucun
/// `Content-Disposition` avec `name=` n'est trouvé (part malformé — ignoré
/// plutôt que de planter).
fn parse_one_multipart_part(segment: &[u8]) -> Option<MultipartPart> {
    let (header_end, sep_len) = find_subslice(segment, b"\r\n\r\n").map(|i| (i, 4))
        .or_else(|| find_subslice(segment, b"\n\n").map(|i| (i, 2)))?;
    let header_text = String::from_utf8_lossy(&segment[..header_end]);
    let body = segment[header_end + sep_len..].to_vec();

    let mut name: Option<String> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;

    for line in header_text.split(['\r', '\n']) {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(rest) = strip_ci_prefix(line, "Content-Disposition:") {
            for attr in rest.split(';').skip(1) {
                let attr = attr.trim();
                if let Some(v) = strip_ci_prefix(attr, "name=") {
                    name = Some(v.trim().trim_matches('"').to_string());
                } else if let Some(v) = strip_ci_prefix(attr, "filename=") {
                    filename = Some(v.trim().trim_matches('"').to_string());
                }
            }
        } else if let Some(rest) = strip_ci_prefix(line, "Content-Type:") {
            content_type = Some(rest.trim().to_string());
        }
    }

    Some(MultipartPart { name: name?, filename, content_type, body })
}

/// Construit les 10 buckets de `params()` (voir `METHOD_BUCKETS`) à partir de
/// la query string de l'URL (TOUJOURS dans le bucket "GET", quelle que soit
/// la méthode réelle) et, si applicable, du corps de la requête (urlencoded
/// ou multipart) dans le bucket de la méthode RÉELLE — fusionné dans "GET" si
/// la méthode réelle est justement `GET` (cas rare : une requête GET avec un
/// corps ET une query string). Calculé UNE SEULE FOIS par requête (voir son
/// appel dans `handle_request`), consulté ensuite par `param()`/`params()`
/// sans jamais re-parser.
pub(crate) fn build_params_buckets(
    query_str: &str,
    method: &str,
    headers: &HashMap<String, String>,
    body: &[u8],
    path_params: &HashMap<String, ParamValue>,
) -> HashMap<String, HashMap<String, ParamValue>> {
    let mut buckets: HashMap<String, HashMap<String, ParamValue>> = METHOD_BUCKETS.iter()
        .map(|m| (m.to_string(), HashMap::new()))
        .collect();

    // Query string : toujours dans "GET", indépendamment de la méthode réelle
    // (une query string peut légitimement accompagner N'IMPORTE QUELLE méthode).
    for (k, v) in parse_query(query_str) {
        buckets.get_mut("GET").unwrap().insert(k, ParamValue::Text(v));
    }

    // Paramètres de CHEMIN (`<id:int>`, voir `match_route`) : insérés APRÈS
    // la query string, dans le MÊME bucket "GET" — l'emportent donc sur la
    // query string à clé égale (voir docs/roadmap.d/stdlib-httpserver-route-params.md,
    // décision confirmée : un paramètre de chemin est plus spécifique/
    // intentionnel qu'une query string arbitraire portant le même nom).
    for (k, v) in path_params {
        buckets.get_mut("GET").unwrap().insert(k.clone(), v.clone());
    }

    if body.is_empty() || !METHOD_BUCKETS.contains(&method) {
        return buckets;
    }
    let Some(content_type) = content_type_of(headers) else { return buckets; };
    let target = method; // "GET" fusionne naturellement (même bucket que la query string)

    let ct_lower = content_type.to_ascii_lowercase();
    if ct_lower.starts_with("application/x-www-form-urlencoded") {
        if let Ok(body_str) = std::str::from_utf8(body) {
            let bucket = buckets.entry(target.to_string()).or_default();
            for (k, v) in parse_query(body_str) {
                // Sur GET+corps (cas rare), le corps écrase la query string à
                // clé égale — dernier appel à `insert` gagnant, cohérent avec
                // la règle de précédence "le corps l'emporte" de `param()`.
                bucket.insert(k, ParamValue::Text(v));
            }
        }
    } else if let Some(boundary) = parse_boundary(content_type) {
        let bucket = buckets.entry(target.to_string()).or_default();
        for part in parse_multipart(body, &boundary) {
            let value = match part.filename {
                Some(filename) => ParamValue::File {
                    filename,
                    content_type: part.content_type.unwrap_or_else(|| "application/octet-stream".to_string()),
                    content: part.body,
                },
                None => ParamValue::Text(String::from_utf8_lossy(&part.body).to_string()),
            };
            bucket.insert(part.name, value);
        }
    }
    // Content-Type non reconnu (ex. application/json — géré par l'appelant
    // via JSON::decode(req.body()), explicitement hors périmètre ici) : le
    // bucket de la méthode réelle reste tel quel (vide, sauf fusion GET
    // ci-dessus si target == "GET").

    buckets
}

/// Résout `param(key, method)` — voir la doc complète dans
/// `HTTPServerRequest_param`/`_param_1` : soit une recherche DIRECTE dans un
/// bucket précis (`method` donné), soit la fusion GET/méthode réelle avec le
/// corps prioritaire sur la query string (`method` = `None`, valeur par
/// défaut Ocara).
pub(crate) fn lookup_param<'a>(
    buckets: &'a HashMap<String, HashMap<String, ParamValue>>,
    key: &str,
    method: Option<&str>,
    actual_method: &str,
) -> Option<&'a ParamValue> {
    match method {
        Some(m) => buckets.get(m).and_then(|b| b.get(key)),
        None => {
            let from_get = buckets.get("GET").and_then(|b| b.get(key));
            if actual_method == "GET" {
                return from_get;
            }
            // Le corps (bucket de la méthode réelle) l'emporte sur la query
            // string en cas de collision — voir la doc de la spécification.
            buckets.get(actual_method).and_then(|b| b.get(key)).or(from_get)
        }
    }
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
        None => {
            server_log!("[STATIC] No root_path configured\n");
            return false;
        }
    };

    // Nettoyer le chemin : retirer le / initial et décoder
    let clean_path = req_path.trim_start_matches('/');
    
    // Protection contre path traversal : bloquer ../ ou ../
    if clean_path.contains("..") {
        server_log!("[STATIC] Path traversal attempt blocked\n");
        return false;
    }

    // Construire le chemin complet
    let file_path = std::path::Path::new(root).join(clean_path);
    
    // Vérifier que le fichier existe
    if !file_path.exists() {
        server_log!("[STATIC] file does not exist\n");
        return false;
    }
    
    // Vérifier que le fichier canonique reste dans le root (double protection)
    // Note : root est déjà canonicalisé par HTTPServer_rootPath
    match file_path.canonicalize() {
        Ok(canonical) => {
            // Vérifier que le fichier est bien dans le root
            let root_path_obj = std::path::Path::new(root);
            if !canonical.starts_with(root_path_obj) {
                server_log!("[STATIC] File is outside root directory\n");
                return false;
            }
        }
        Err(e) => {
            server_log!("[STATIC] Failed to canonicalize file path: {:?} - {}\n", file_path, e);
            return false;
        }
    }

    // Tenter de lire le fichier
    let content = match std::fs::read(&file_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            server_log!("[STATIC] Failed to read file: {}\n", e);
            return false;
        }
    };

    // Déterminer le MIME type
    let mime = mime_type_from_path(clean_path);

    // Remplir la réponse
    let ctx = unsafe { ctx_ref(req_handle) };
    ctx.resp_status = 200;
    ctx.resp_body = content;
    
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

/// Verrou des handlers, commun à tous les serveurs — voir la note sécurité
/// concurrente en tête de fichier.
static HANDLER_LOCK: Mutex<()> = Mutex::new(());

/// Configuration d'un serveur démarré, partagée par ses workers et par les
/// threads de réception de corps (voir `dispatch_request`).
struct ServerShared {
    routes:         Vec<Route>,
    root_path:      Option<String>,
    error_handlers: HashMap<u16, SendHandler>,
    max_body:       usize,
    receiving:      AtomicUsize,
}

/// Exécute un handler Ocara sous la protection de `HANDLER_LOCK` — voir la
/// note sécurité concurrente en tête de fichier : sérialise l'invocation
/// pour qu'aucune capture partagée (heap_promoted) ne soit jamais touchée
/// par deux handlers en même temps. Une exception non rattrapée dans le
/// handler est capturée ici (le serveur continue) : `false`, l'appelant
/// répond 500.
unsafe fn call_handler_locked(h: &SendHandler, req_handle: i64) -> bool {
    let _guard = {
        let _parked = crate::rc::park();
        HANDLER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    };
    match crate::run_closure_catching_with_arg(h.func_ptr, h.env_ptr, req_handle) {
        Ok(_) => true,
        Err((error_val, error_type)) => {
            let type_name = if error_type == 0 { "" } else { unsafe { ptr_to_str(error_type) } };
            let type_name = type_name.split('|').next().unwrap_or("");
            server_log!("HTTPServer: unhandled exception in handler ({}) → 500\n", type_name);
            crate::rc::__rc_release(error_val);
            false
        }
    }
}

/// Répond `code` : handler `routeError(code)` s'il existe (lui aussi protégé
/// contre les exceptions, 500 par défaut s'il échoue), sinon `default_body`.
fn respond_error(req_handle: i64, code: u16, default_body: String, error_handlers: &HashMap<u16, SendHandler>) {
    let ctx = unsafe { ctx_ref(req_handle) };
    ctx.resp_status = code;
    ctx.resp_body = default_body.into_bytes();
    if let Some(error_h) = error_handlers.get(&code) {
        if !unsafe { call_handler_locked(error_h, req_handle) } {
            reset_response(req_handle, 500, "500 Internal Server Error");
        }
    }
}

fn reset_response(req_handle: i64, code: u16, body: &str) {
    let ctx = unsafe { ctx_ref(req_handle) };
    ctx.resp_status = code;
    ctx.resp_body = body.as_bytes().to_vec();
    ctx.resp_headers.clear();
}

fn respond_status(request: tiny_http::Request, code: u16, body: &str) {
    let _parked = crate::rc::park();
    let _ = request.respond(tiny_http::Response::from_string(body).with_status_code(code));
}

/// Un corps de plus de 1 Kio (ou `chunked`) est lu depuis la socket par
/// l'appelant de `as_reader()` — tiny_http a déjà lu les plus petits.
fn body_is_streamed(request: &tiny_http::Request) -> bool {
    match request.body_length() {
        Some(n) => n > 1024,
        None => request.headers().iter().any(|h| h.field.equiv("Transfer-Encoding")),
    }
}

/// Un corps à lire depuis la socket est reçu hors des workers, dans un thread
/// dédié (borné par `MAX_RECEIVING_BODIES`) : N clients qui téléversent
/// lentement ne bloquent plus les autres requêtes.
fn dispatch_request(request: tiny_http::Request, shared: &Arc<ServerShared>) {
    if !body_is_streamed(&request) {
        handle_request(request, shared);
        return;
    }
    if shared.receiving.fetch_add(1, Ordering::SeqCst) >= MAX_RECEIVING_BODIES {
        shared.receiving.fetch_sub(1, Ordering::SeqCst);
        respond_status(request, 503, "Service Unavailable");
        return;
    }
    let shared = Arc::clone(shared);
    let guard = crate::rc::ThreadGuard::new();
    std::thread::spawn(move || {
        let _guard = guard;
        let _stack = crate::stack_guard::ThreadStack::enter();
        handle_request(request, &shared);
        shared.receiving.fetch_sub(1, Ordering::SeqCst);
    });
}

/// Lit le corps (borné par `max_body` et par `BODY_DEADLINE`, chaque lecture
/// par `IDLE_TIMEOUT`) — `Err(code)` : 413 trop grand, 408 trop lent ou
/// connexion interrompue.
fn read_body(request: &mut tiny_http::Request, max_body: usize) -> Result<Vec<u8>, u16> {
    if max_body > 0 && request.body_length().is_some_and(|n| n > max_body) {
        return Err(413);
    }
    let _parked = crate::rc::park();
    let started = Instant::now();
    let reader = request.as_reader();
    let mut body: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        if started.elapsed() > BODY_DEADLINE {
            return Err(408);
        }
        match reader.read(&mut chunk) {
            Ok(0) => return Ok(body),
            Ok(n) => {
                body.extend_from_slice(&chunk[..n]);
                if max_body > 0 && body.len() > max_body {
                    return Err(413);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(408),
        }
    }
}

/// Route de la requête : méthode exacte d'abord, puis la route `GET` pour un
/// `HEAD` (tiny_http n'envoie pas le corps d'une réponse à `HEAD`). `Err`
/// liste les méthodes acceptées par ce chemin (vide : aucune route, 404 ;
/// sinon 405).
fn find_route(routes: &[Route], method: &str, path: &str) -> Result<(SendHandler, HashMap<String, ParamValue>), Vec<String>> {
    let lookup = |wanted: &str| routes.iter()
        .filter(|r| r.method == wanted)
        .find_map(|r| match_route(&r.pattern, path).map(|params| (r.handler.clone(), params)));
    if let Some(found) = lookup(method).or_else(|| if method == "HEAD" { lookup("GET") } else { None }) {
        return Ok(found);
    }
    let mut allowed: Vec<String> = routes.iter()
        .filter(|r| !matches!(r.pattern, RoutePattern::Wildcard) && match_route(&r.pattern, path).is_some())
        .map(|r| r.method.clone())
        .collect();
    allowed.sort();
    allowed.dedup();
    Err(allowed)
}

fn handle_request(mut request: tiny_http::Request, shared: &ServerShared) {
    // Lire le corps en OCTETS BRUTS (pas `read_to_string`) : un corps
    // `multipart/form-data` contenant un fichier (JPEG, PDF...) n'est PAS de
    // l'UTF-8 valide. `ctx.body` (accessible côté Ocara via `body(): string`)
    // reste dérivé en texte via une conversion LOSSY — comportement inchangé
    // pour un corps texte réel (JSON, urlencoded...).
    let raw_body = match read_body(&mut request, shared.max_body) {
        Ok(body) => body,
        Err(413) => return respond_status(request, 413, "Payload Too Large"),
        Err(code) => return respond_status(request, code, "Request Timeout"),
    };
    let body = String::from_utf8_lossy(&raw_body).to_string();

    let full_url = request.url().to_string();
    let (path, query_str) = split_target(&full_url);

    let method = request.method().to_string().to_uppercase();

    // Casse D'ORIGINE préservée (voir la doc de `OcaraHttpContext.headers`) ;
    // un en-tête répété (deux `Cookie`) est fusionné, jamais écrasé.
    let mut headers: HashMap<String, String> = HashMap::new();
    for h in request.headers() {
        insert_request_header(&mut headers, h.field.as_str().as_str(), h.value.as_str());
    }

    let route = find_route(&shared.routes, &method, path);
    let empty_params = HashMap::new();
    let path_params = match &route {
        Ok((_, params)) => params,
        Err(_) => &empty_params,
    };

    // Paramètres GET (query string PUIS paramètres de chemin, qui l'emportent
    // sur la query string à clé égale — voir `build_params_buckets`) + corps
    // (urlencoded/multipart, méthode réelle) — calculés une seule fois ici.
    let param_buckets = build_params_buckets(query_str, &method, &headers, &raw_body, path_params);

    let path_str = path.to_string();
    let ctx = Box::new(OcaraHttpContext {
        path:         path_str.clone(),
        method:       method.clone(),
        body,
        headers,
        query:        parse_query(query_str),
        param_buckets,
        resp_status:  200,
        resp_body:    Vec::new(),
        resp_headers: Vec::new(),
        session_id:   None,
        request:      Some(request),
    });
    let req_handle = crate::rc::handle_new(*ctx);

    match route {
        Ok((handler, _)) => {
            if !unsafe { call_handler_locked(&handler, req_handle) } {
                reset_response(req_handle, 500, "500 Internal Server Error");
                respond_error(req_handle, 500, "500 Internal Server Error".into(), &shared.error_handlers);
            }
        }
        Err(allowed) => {
            // GET / sans route → /index.html ; chemin décodé (`%C3%A9.txt`).
            let serve_path = if path_str == "/" { "/index.html".to_string() } else { percent_decode(&path_str) };
            let is_read = method == "GET" || method == "HEAD";
            let served = is_read && try_serve_static_file(req_handle, &serve_path, shared.root_path.as_deref());
            if served {
                // fichier statique
            } else if allowed.is_empty() {
                respond_error(req_handle, 404, format!("404 Not Found: {} {}", method, path_str), &shared.error_handlers);
            } else {
                if let Ok(h) = tiny_http::Header::from_bytes(&b"Allow"[..], allowed.join(", ").as_bytes()) {
                    unsafe { ctx_ref(req_handle) }.resp_headers.push(h);
                }
                respond_error(req_handle, 405, format!("405 Method Not Allowed: {} {}", method, path_str), &shared.error_handlers);
            }
        }
    }

    // Envoyer la réponse
    let ctx = unsafe { &mut *(req_handle as *mut OcaraHttpContext) };

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
        let mut resp = tiny_http::Response::from_data(std::mem::take(&mut ctx.resp_body))
            .with_status_code(tiny_http::StatusCode(ctx.resp_status));
        for h in ctx.resp_headers.drain(..) {
            resp = resp.with_header(h);
        }
        let _parked = crate::rc::park();
        let _ = req.respond(resp);
    }

    drop(unsafe { crate::rc::handle_take::<OcaraHttpContext>(req_handle) });
}

// ─────────────────────────────────────────────────────────────────────────────
// API publique exportée (convention C)
// ─────────────────────────────────────────────────────────────────────────────

/// Constructeur : alloue un OcaraHttpServer et écrit le pointeur dans le slot.
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_init(self_ptr: i64) {
    let s   = Box::new(OcaraHttpServer::new());
    let raw = Box::into_raw(s) as i64;
    unsafe { *(self_ptr as *mut i64) = raw; }
}

/// Définit le port d'écoute (défaut : 8080).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_port(self_ptr: i64, port: i64) {
    let s = unsafe { server_from_slot(self_ptr) };
    s.port = port as u16;
}

/// Définit l'adresse d'écoute (défaut : "0.0.0.0").
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_host(self_ptr: i64, host_ptr: i64) {
    let s = unsafe { server_from_slot(self_ptr) };
    s.host = unsafe { ptr_to_str(host_ptr).to_string() };
}

/// Taille maximale du corps de requête en octets (défaut : 10 Mio, 0 : sans
/// limite). Au-delà, le serveur répond 413 sans appeler le handler.
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_maxBodySize(self_ptr: i64, bytes: i64) {
    let s = unsafe { server_from_slot(self_ptr) };
    s.max_body = bytes.max(0) as usize;
}

/// Définit le nombre de threads workers (défaut : 4).
/// Durée d'inactivité (secondes) après laquelle une session expire
/// (défaut 30 min, 0 : jamais) — réglage commun à tout le processus, comme
/// le stockage des sessions (voir `httpsession`).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_sessionTimeout(_self_ptr: i64, seconds: i64) {
    crate::httpsession::set_session_timeout(seconds);
}

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_workers(self_ptr: i64, n: i64) {
    let s = unsafe { server_from_slot(self_ptr) };
    s.workers = n.max(1) as usize;
}

/// Définit le répertoire racine pour servir les fichiers statiques.
/// Si défini, les requêtes qui ne matchent aucune route tenteront de servir
/// un fichier depuis ce répertoire. Protégé contre path traversal.
/// Le chemin est automatiquement résolu en chemin absolu si possible.
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_rootPath(self_ptr: i64, path_ptr: i64) {
    let s = unsafe { server_from_slot(self_ptr) };
    let path = unsafe { ptr_to_str(path_ptr).to_string() };
    
    if path.is_empty() {
        s.root_path = None;
        return;
    }
    
    // Tenter de résoudre le chemin en absolu depuis le répertoire courant
    let path_buf = std::path::Path::new(&path);
    let resolved = if path_buf.is_absolute() {
        // Déjà absolu
        path
    } else {
        // Relatif : résoudre depuis le répertoire courant
        match std::env::current_dir() {
            Ok(cwd) => {
                let full_path = cwd.join(&path);
                match full_path.canonicalize() {
                    Ok(canonical) => {
                        let canonical_str = canonical.to_string_lossy().to_string();
                        canonical_str
                    }
                    Err(_) => {
                        path
                    }
                }
            }
            Err(_) => {
                path
            }
        }
    };
    
    s.root_path = Some(resolved);
}

/// Enregistre une route. `path` peut contenir des segments paramétrés et
/// typés `<nom:type>` (`int`/`float`/`bool`/`string`, ex. `/voitures/<id:int>`
/// — voir docs/roadmap.d/stdlib-httpserver-route-params.md) : compilé UNE
/// SEULE FOIS ici en `RoutePattern` (`parse_route_pattern`), jamais reparsé
/// par requête (voir `match_route`, appelée depuis `handle_request`). Un
/// paramètre matché est exposé côté Ocara via `req.param(nom)`/`req.params()`
/// (bucket `"GET"`, où il l'emporte sur une query string de même clé).
/// `fat_ptr` pointe sur un struct {func_ptr: i64, env_ptr: i64} (fat pointer Ocara).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_route(
    self_ptr: i64,
    path_ptr: i64,
    method_ptr: i64,
    fat_ptr: i64,
) {
    let s          = unsafe { server_from_slot(self_ptr) };
    crate::rc::__rc_retain(fat_ptr);
    let func_ptr   = unsafe { *(fat_ptr as *const i64) };
    let env_ptr    = unsafe { *((fat_ptr as *const i64).add(1)) };
    let path   = unsafe { ptr_to_str(path_ptr).to_string() };
    let method = unsafe { ptr_to_str(method_ptr).to_string().to_uppercase() };
    s.routes.push(Route {
        pattern: parse_route_pattern(&path),
        method,
        handler: SendHandler { func_ptr, env_ptr },
    });
}

/// Enregistre un handler pour un code d'erreur HTTP spécifique.
/// Permet de personnaliser les pages d'erreur (404, 500, etc.).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_routeError(
    self_ptr: i64,
    code: i64,
    fat_ptr: i64,
) {
    let s        = unsafe { server_from_slot(self_ptr) };
    crate::rc::__rc_retain(fat_ptr);
    let func_ptr = unsafe { *(fat_ptr as *const i64) };
    let env_ptr  = unsafe { *((fat_ptr as *const i64).add(1)) };
    s.error_handlers.insert(code as u16, SendHandler { func_ptr, env_ptr });
}

/// Démarre le serveur (appel bloquant).
/// Lance `workers` threads qui acceptent les connexions en parallèle.
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServer_run(self_ptr: i64) {
    let data   = unsafe { server_from_slot(self_ptr) };
    let addr   = format!("{}:{}", data.host, data.port);
    // Délai par connexion acceptée (tiny_http patché, voir le Cargo.toml
    // racine) : un client muet reçoit 408 au lieu de bloquer un thread.
    let server = match std::net::TcpListener::bind(&addr).map_err(|e| e.to_string())
        .and_then(|listener| tiny_http::Server::from_listener_with_timeout(listener, None, Some(IDLE_TIMEOUT)).map_err(|e| e.to_string()))
    {
        Ok(s)  => Arc::new(s),
        Err(e) => {
            unsafe {
                throw_httpserver_exception(
                    &format!("Unable to start HTTPServer on {}: {}", addr, e),
                    ERR_SERVER_START
                );
            }
        }
    };
    let shared = Arc::new(ServerShared {
        routes:         std::mem::take(&mut data.routes),
        root_path:      data.root_path.clone(),
        error_handlers: std::mem::take(&mut data.error_handlers),
        max_body:       data.max_body,
        receiving:      AtomicUsize::new(0),
    });

    server_log!("HTTPServer: listening on http://{}\n", addr);

    let handles: Vec<_> = (0..data.workers).map(|_| {
        let server = Arc::clone(&server);
        let shared = Arc::clone(&shared);
        let guard = crate::rc::ThreadGuard::new();
        std::thread::spawn(move || {
            let _guard = guard;
            let _stack = crate::stack_guard::ThreadStack::enter();
            loop {
                let next = { let _parked = crate::rc::park(); server.recv() };
                match next {
                    Ok(request) => dispatch_request(request, &shared),
                    Err(_)      => break,
                }
            }
        })
    }).collect();

    let _parked = crate::rc::park();
    for h in handles {
        let _ = h.join();
    }
}

// ─── HTTPServerRequest — méthodes d'instance, lecture de la requête ──────────
// (voir docs/roadmap.d/stdlib-httpserver-request-object.md, désormais clos —
// remplace les anciennes méthodes STATIQUES `HTTPServer::path/method/body/
// header/query/respond/respondHeader(req, ...)`, cassé volontairement sans
// période de coexistence, comme demandé : `req:int` ne compile plus.)

/// Retourne le chemin de la requête courante (sans query string).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_path(req: i64) -> i64 {
    let path = unsafe { ctx_ref(req).path.clone() };
    unsafe { alloc_str(&path) }
}

/// Retourne la méthode HTTP de la requête courante (ex: "GET").
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_method(req: i64) -> i64 {
    let method = unsafe { ctx_ref(req).method.clone() };
    unsafe { alloc_str(&method) }
}

/// Retourne le corps BRUT de la requête courante (conversion lossy si le
/// corps n'est pas de l'UTF-8 valide — voir `handle_request` ; un corps
/// multipart contenant un fichier binaire doit être lu via `param()`/
/// `params()`, pas `body()`).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_body(req: i64) -> i64 {
    let body = unsafe { ctx_ref(req).body.clone() };
    unsafe { alloc_str(&body) }
}

/// Retourne la valeur d'un en-tête de la requête — recherche INSENSIBLE à la
/// casse (voir `header_lookup`). Retourne une chaîne vide si l'en-tête est
/// absent (comportement inchangé par rapport à l'ancien `HTTPServer::header`).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_header(req: i64, name_ptr: i64) -> i64 {
    let name = unsafe { ptr_to_str(name_ptr).to_string() };
    let val  = header_lookup(&unsafe { ctx_ref(req) }.headers, &name)
        .unwrap_or("")
        .to_string();
    unsafe { alloc_str(&val) }
}

/// Retourne TOUS les en-têtes de la requête, clés dans leur casse D'ORIGINE
/// (voir la doc de `OcaraHttpContext.headers`) — contrairement à `header()`,
/// jamais insensible à la casse ici : c'est la recherche par NOM qui l'est,
/// pas les clés de cette map. Chaque valeur est une string (les en-têtes HTTP
/// sont toujours du texte sur le fil) — le type de retour déclaré côté Ocara
/// (`map<string, string|int|float|bool|null>`) n'est qu'une parité d'API avec
/// `params()`, jamais réellement peuplé d'autre chose qu'une string ici.
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_headers(req: i64) -> i64 {
    let ctx = unsafe { ctx_ref(req) };
    let map = crate::__map_new();
    for (k, v) in &ctx.headers {
        let key = unsafe { alloc_str(k) };
        let val = unsafe { alloc_str(v) };
        crate::map_set_owned_key(map, key, val);
    }
    map
}

/// Retourne la valeur d'un paramètre query string. Retourne une chaîne vide
/// si le paramètre est absent (comportement inchangé — `query()` reste
/// distinct de `param()`, qui lui suit la convention `mixed`/absent = `0`).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_query(req: i64, key_ptr: i64) -> i64 {
    let key = unsafe { ptr_to_str(key_ptr).to_string() };
    let val = unsafe { ctx_ref(req) }.query.get(&key)
        .cloned()
        .unwrap_or_default();
    unsafe { alloc_str(&val) }
}

/// Convertit une `ParamValue` en valeur `mixed` Ocara auto-décrite —
/// `Text` → string (Ptr, jamais boxée : une string EST déjà `mixed`-shaped) ;
/// `File` → `map<string,mixed>` construite nativement (voir `file_to_mixed_map`).
unsafe fn param_value_to_mixed(v: &ParamValue) -> i64 {
    match v {
        ParamValue::Text(s) => unsafe { alloc_str(s) },
        // Un paramètre de CHEMIN typé (`<id:int>` etc., voir `match_route`)
        // — mêmes primitives de boxing que partout ailleurs dans ce fichier/
        // ce runtime (jamais un second chemin de boxing inventé pour
        // l'occasion, voir docs/roadmap.d/stdlib-sqlite-integer-column-boxing.md
        // pour le bug que ça évite : un `int` non boxé logé dans un `mixed`
        // est ambigu avec un pointeur dès qu'il dépasse `PTR_THRESHOLD`).
        ParamValue::Int(n) => crate::box_int_if_needed(*n),
        ParamValue::Float(f) => crate::__box_float(f.to_bits() as i64),
        ParamValue::Bool(b) => crate::__box_bool(if *b { 1 } else { 0 }),
        ParamValue::File { filename, content_type, content } => unsafe {
            file_to_mixed_map(filename, content_type, content)
        },
    }
}

/// Construit la `map<string,mixed>` d'un champ fichier multipart — clés
/// `filename:string`, `contentType:string`, `size:int`, `content:array<int>`.
///
/// Choix pour `content` (DÉCISION DOCUMENTÉE — voir
/// docs/roadmap.d/stdlib-httpserver-request-object.md) : `array<int>` (un
/// octet par élément), PAS `string` malgré la spec initiale qui suggérait
/// `content:string` — `ptr_to_str` (runtime/src/lib.rs) exige de l'UTF-8
/// valide et retombe SILENCIEUSEMENT sur `""` sinon (`std::str::from_utf8(...)
/// .unwrap_or("")`), ce qui aurait corrompu tout upload binaire réel (image,
/// PDF...) dès sa première lecture côté Ocara. `array<int>` est la convention
/// binaire-sûre DÉJÀ établie par ce projet pour exactement ce cas
/// (`File::readBytes`/`writeBytes`, voir src/builtins/file.rs et
/// runtime/src/file.rs::File_readBytes) — réutilisée ici plutôt que
/// d'inventer une troisième convention (ex. base64).
///
/// `size` est boxé (`box_int_if_needed`) : c'est un `int` logé dans un
/// `mixed` (la map résultat), et un fichier réel dépasse trivialement
/// `PTR_THRESHOLD` (0x10000 = 64 Kio) — même invariant, même bug potentiel
/// (SIGSEGV) que docs/roadmap.d/stdlib-sqlite-integer-column-boxing.md si
/// omis ici.
unsafe fn file_to_mixed_map(filename: &str, content_type: &str, content: &[u8]) -> i64 {
    let map = crate::__map_new();
    unsafe {
        crate::map_set_owned_key(map, alloc_str("filename"), alloc_str(filename));
        crate::map_set_owned_key(map, alloc_str("contentType"), alloc_str(content_type));
        crate::map_set_owned_key(map, alloc_str("size"), crate::box_int_if_needed(content.len() as i64));
        let bytes_arr = crate::rc::__rc_mark_raw(crate::__array_new());
        for byte in content {
            crate::array_push_owned(bytes_arr, *byte as i64);
        }
        crate::map_set_owned_key(map, alloc_str("content"), bytes_arr);
    }
    map
}

/// `req.param(key)` — équivalent à `req.param(key, null)`, voir `param()`
/// complet ci-dessous pour la règle de précédence.
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_param_1(req: i64, key_ptr: i64) -> i64 {
    let key = unsafe { ptr_to_str(key_ptr).to_string() };
    let ctx = unsafe { ctx_ref(req) };
    match lookup_param(&ctx.param_buckets, &key, None, &ctx.method) {
        Some(v) => unsafe { param_value_to_mixed(v) },
        // Absent : même convention que __map_get sur une clé manquante
        // (runtime/src/lib.rs) — `0`, jamais une valeur inventée.
        None => 0,
    }
}

/// `req.param(key, method = null)` — accessoir universel `mixed`.
///
/// - `method` donné (normalisé en MAJUSCULES, comparaison insensible à la
///   casse — `"post"`/`"POST"` équivalents) : recherche UNIQUEMENT dans le
///   bucket de cette méthode (voir `params()`/`METHOD_BUCKETS`).
/// - `method` = `null` (0) : fusionne le bucket "GET" (query string) avec
///   celui de la méthode RÉELLE de la requête (corps), le corps l'emportant
///   en cas de collision de clé — si la méthode réelle EST "GET", il n'y a
///   qu'un seul bucket à consulter (la fusion est un no-op).
/// - Absent : `0` (même convention que `__map_get` sur une clé manquante).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_param(req: i64, key_ptr: i64, method_ptr: i64) -> i64 {
    let key = unsafe { ptr_to_str(key_ptr).to_string() };
    let ctx = unsafe { ctx_ref(req) };
    let method_upper = if method_ptr == 0 {
        None
    } else {
        Some(unsafe { ptr_to_str(method_ptr) }.to_uppercase())
    };
    match lookup_param(&ctx.param_buckets, &key, method_upper.as_deref(), &ctx.method) {
        Some(v) => unsafe { param_value_to_mixed(v) },
        None => 0,
    }
}

/// `req.params(): map<string, map<string, mixed>>` — les 10 buckets toujours
/// présents (voir `METHOD_BUCKETS`), construits nativement (comme
/// `collect_all_rows` pour SQLite) plutôt que via une indexation Ocara-level
/// `m[clé] = valeur` — voir la doc de `ParamValue`/`param_value_to_mixed`
/// pour le boxing correct de chaque valeur (délibérément PAS construit en
/// générant du code Ocara `m[k]=v`, qui avait son propre bug de boxing
/// distinct — voir docs/roadmap.d/langage-mixed-container-indexed-assignment-boxing.md
/// — cette fonction s'en affranchit entièrement en construisant tout côté
/// Rust).
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_params(req: i64) -> i64 {
    let ctx = unsafe { ctx_ref(req) };
    let outer = crate::__map_new();
    for bucket_name in METHOD_BUCKETS {
        let inner = crate::__map_new();
        if let Some(bucket) = ctx.param_buckets.get(bucket_name) {
            for (k, v) in bucket {
                let key = unsafe { alloc_str(k) };
                let val = unsafe { param_value_to_mixed(v) };
                crate::map_set_owned_key(inner, key, val);
            }
        }
        let outer_key = unsafe { alloc_str(bucket_name) };
        crate::map_set_owned_key(outer, outer_key, inner);
    }
    outer
}

// ─── HTTPServerRequest — méthodes d'instance, construction de la réponse ─────

/// Définit le statut HTTP et le corps de la réponse.
/// Peut être appelé plusieurs fois : seul le dernier appel est utilisé.
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_respond(req: i64, status: i64, body_ptr: i64) {
    let body      = unsafe { ptr_to_str(body_ptr).to_string() };
    let ctx       = unsafe { ctx_ref(req) };
    ctx.resp_status = status as u16;
    ctx.resp_body   = body.into_bytes();
}

/// Ajoute un en-tête à la réponse (ex: "Content-Type", "text/html").
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_respondHeader(req: i64, name_ptr: i64, value_ptr: i64) {
    let name  = unsafe { ptr_to_str(name_ptr).to_string() };
    let value = unsafe { ptr_to_str(value_ptr).to_string() };
    if !is_valid_header_name(&name) || !is_valid_header_value(&value) {
        unsafe {
            throw_httpserver_exception(
                &format!("Invalid response header '{}': CR, LF and control characters are not allowed", name.escape_debug()),
                ERR_INVALID_HEADER,
            );
        }
    }
    let ctx = unsafe { ctx_ref(req) };
    if let Ok(h) = tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()) {
        ctx.resp_headers.push(h);
    }
}
