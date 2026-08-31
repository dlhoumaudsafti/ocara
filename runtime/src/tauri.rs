
// runtime/src/tauri.rs — squelette structuré pour intégration Tauri native
// Ce fichier prépare tous les points d'extension pour brancher la logique Tauri (Rust natif)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

use crate::{alloc_str, ptr_to_str, __map_get};

/// Codes d'erreur TauriException (voir docs/builtins/Tauri.md)
const ERR_TAURI_DUPLICATE_HANDLER: i64 = 101;

/// Un handler JS→Ocara enregistré : l'adresse du trampoline généré par le
/// compilateur pour ce couple (nom, fonction), et la liste ORDONNÉE des noms
/// de paramètres Ocara (ex: ["a","b","c"]) — utilisée par le shim JS injecté
/// dans la page (voir tauri_ipc_shim_script) pour convertir un payload array
/// (`invoke(cmd, [v1,v2,v3])`) en objet nommé (`{a:v1,b:v2,c:v3}`) *avant* que
/// Tauri lui-même ne le voie (son pont IPC natif traite un array top-level
/// comme du binaire brut, jamais comme du JSON — voir tauri_ipc_shim_script).
struct HandlerInfo {
    trampoline_addr: i64,
    param_names: Vec<String>,
}

// Structure représentant une fenêtre Tauri côté Ocara
struct OcaraTauriWindow {
    title: String,
    width: i64,
    height: i64,
    url: String,
    is_open: bool,
    is_minimized: bool,
    is_maximized: bool,
    has_focus: bool,
    // Gestion des callbacks JS → Ocara
    event_callbacks: HashMap<String, i64>, // event_name → callback_ptr
    // Handlers JS→Ocara enregistrés via ui.handler(name, fn) / ui.handlers({...}).
    // Contrairement à event_callbacks (fire-and-forget), la valeur ici est
    // l'adresse d'un trampoline *généré par le compilateur* pour CE couple
    // (nom, fonction) précis : il connaît statiquement la signature réelle de
    // la fonction Ocara ciblée, décode les arguments JSON en conséquence, puis
    // appelle la vraie fonction et sérialise son retour. Signature uniforme :
    // extern "C" fn(json_args_ptr: i64) -> i64 (pointeur vers une string JSON
    // "{"ok":true,"value":...}" ou "{"ok":false,"error":"..."}").
    handlers: HashMap<String, HandlerInfo>, // command_name → (trampoline_ptr, noms de paramètres ordonnés)
    // TODO: Ajouter les handles natifs Tauri si besoin
}

// Table globale des fenêtres créées (clé = pointeur Ocara)
// Convention du projet : once_cell::sync::Lazy plutôt que lazy_static (cf. dotenv.rs)
static TAURI_WINDOWS: Lazy<Mutex<HashMap<i64, Arc<Mutex<OcaraTauriWindow>>>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

/// Lit une clé string d'une map d'options (`__map_get` + décodage du pointeur
/// string), ou renvoie `default` si la clé est absente (valeur brute nulle).
unsafe fn map_get_str(options_ptr: i64, key: &str, default: &str) -> String {
    let key_ptr = unsafe { alloc_str(key) };
    let val = __map_get(options_ptr, key_ptr);
    if val == 0 {
        default.to_string()
    } else {
        unsafe { ptr_to_str(val).to_string() }
    }
}

/// Lit une clé int d'une map d'options, ou renvoie `default` si absente (0).
unsafe fn map_get_int(options_ptr: i64, key: &str, default: i64) -> i64 {
    let key_ptr = unsafe { alloc_str(key) };
    let val = __map_get(options_ptr, key_ptr);
    if val == 0 { default } else { val }
}

/// Crée une nouvelle fenêtre Tauri avec les options (title, width, height, url...).
///
/// `this` est le pointeur alloué par `use Tauri(...)` (voir Expr::New dans
/// src/lower/expr.d/lower.rs, qui appelle systématiquement `<Classe>_init(this, ...)` —
/// c'est pourquoi ce constructeur doit s'appeler `Tauri_init`, pas `Tauri___init__` :
/// tout autre nom est silencieusement ignoré par le codegen, qui déclare l'appel
/// vers `Tauri_init` sans jamais vérifier qu'un symbole portant ce nom existe).
/// On stocke l'état de la fenêtre dans TAURI_WINDOWS en utilisant CE pointeur comme
/// clé — c'est lui, et lui seul, qui est repassé en `this` à tous les appels
/// `ui.method()` suivants.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_init(this: i64, options_ptr: i64) {
    let win = Arc::new(Mutex::new(OcaraTauriWindow {
        title:  unsafe { map_get_str(options_ptr, "title", "Ocara App") },
        width:  unsafe { map_get_int(options_ptr, "width", 800) },
        height: unsafe { map_get_int(options_ptr, "height", 600) },
        url:    unsafe { map_get_str(options_ptr, "url", "index.html") },
        is_open: true,
        is_minimized: false,
        is_maximized: false,
        has_focus: true,
        event_callbacks: HashMap::new(),
        handlers: HashMap::new(),
    }));
    TAURI_WINDOWS.lock().unwrap().insert(this, win);
}

/// Enregistre un handler d'événement JS → Ocara
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_listen(this: i64, event_ptr: i64, callback_ptr: i64) {
    let event = unsafe { ptr_to_str(event_ptr).to_string() };
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().event_callbacks.insert(event, callback_ptr);
    }
}

/// Émet un événement Ocara → JS
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_emit(this: i64, event_ptr: i64, data_ptr: i64) {
    let event = unsafe { ptr_to_str(event_ptr as i64).to_string() };
    let data = unsafe { ptr_to_str(data_ptr as i64).to_string() };
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        // Réservé pour un futur état à mettre à jour (dernier événement émis, etc.)
        let _w = win.lock().unwrap();
        // Ici, on pourrait appeler l'API Tauri réelle
        println!("[Tauri_emit] {}: {}", event, data);
    }
}

/// Ouvre une boîte de dialogue native
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_dialog(this: i64, options_ptr: i64) -> i64 {
    let options = unsafe { ptr_to_str(options_ptr as i64).to_string() };
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(_win) = map.get(&this) {
        // Simule une réponse utilisateur
        let response = "ok".to_string();
        println!("[Tauri_dialog] options: {} -> {}", options, response);
        unsafe { alloc_str(&response) }
    } else {
        0
    }
}

/// Envoie une notification système
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_notify(this: i64, options_ptr: i64) {
    let options = unsafe { ptr_to_str(options_ptr as i64).to_string() };
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(_win) = map.get(&this) {
        println!("[Tauri_notify] options: {}", options);
    }
}

// ─────────────────────────────────────────────────────────────
// Méthodes d'accès et gestion de fenêtre (parité avec autres builtins)
// ─────────────────────────────────────────────────────────────

/// Récupère le titre de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getTitle(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let title = win.lock().unwrap().title.clone();
        unsafe { alloc_str(&title) }
    } else {
        0
    }
}

/// Définit le titre de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setTitle(this: i64, title_ptr: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let title = unsafe { ptr_to_str(title_ptr).to_string() };
        win.lock().unwrap().title = title;
    }
}

/// Récupère la largeur de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getWidth(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().width
    } else {
        800
    }
}

/// Définit la largeur de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setWidth(this: i64, width: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().width = width;
    }
}

/// Récupère la hauteur de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getHeight(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().height
    } else {
        600
    }
}

/// Définit la hauteur de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setHeight(this: i64, height: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().height = height;
    }
}

/// Récupère l'URL courante
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getUrl(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let url = win.lock().unwrap().url.clone();
        unsafe { alloc_str(&url) }
    } else {
        0
    }
}

/// Définit l'URL de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setUrl(this: i64, url_ptr: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let url = unsafe { ptr_to_str(url_ptr).to_string() };
        win.lock().unwrap().url = url;
    }
}

/// Ouvre la fenêtre (si minimisée/fermée)
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_open(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_open = true;
        w.is_minimized = false;
    }
}

/// Ferme la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_close(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_open = false;
    }
}

/// Vérifie si la fenêtre est ouverte
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_isOpen(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        if win.lock().unwrap().is_open { 1 } else { 0 }
    } else {
        0
    }
}

/// Met la fenêtre au premier plan
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_focus(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.has_focus = true;
    }
}

/// Minimise la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_minimize(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_minimized = true;
        w.is_maximized = false;
    }
}

/// Maximiser la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_maximize(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_maximized = true;
        w.is_minimized = false;
    }
}

/// Restaure la fenêtre (si minimisée/maximisée)
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_restore(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_minimized = false;
        w.is_maximized = false;
    }
}

/// Vérifie si la fenêtre a le focus
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_hasFocus(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        if win.lock().unwrap().has_focus { 1 } else { 0 }
    } else {
        0
    }
}

/// Vérifie si la fenêtre est minimisée
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_isMinimized(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        if win.lock().unwrap().is_minimized { 1 } else { 0 }
    } else {
        0
    }
}

/// Vérifie si la fenêtre est maximisée
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_isMaximized(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        if win.lock().unwrap().is_maximized { 1 } else { 0 }
    } else {
        0
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers JS→Ocara (ui.handler / ui.handlers) — pont IPC réel
//
// `Tauri_handler_register` est appelée par le compilateur juste après avoir
// généré le trampoline propre à un couple (nom, fonction) donné (voir
// src/lower/expr.d/lower.rs, cas spécial `ui.handler(...)`). Elle se contente
// d'enregistrer l'adresse de ce trampoline sous le nom demandé, en refusant
// tout doublon — l'enregistrement de la fonction elle-même (décodage JSON
// typé, appel réel) est entièrement dans le trampoline généré, pas ici.
// ─────────────────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_handler_register(this: i64, name_ptr: i64, trampoline_addr: i64, param_names_json_ptr: i64) {
    let name = unsafe { ptr_to_str(name_ptr).to_string() };
    let param_names: Vec<String> = {
        let s = unsafe { ptr_to_str(param_names_json_ptr) };
        serde_json::from_str(s).unwrap_or_default()
    };
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        if w.handlers.contains_key(&name) {
            drop(w);
            drop(map);
            unsafe {
                crate::exception::throw_tauri_exception(
                    &format!("un handler nommé '{}' est déjà enregistré (ui.handler/ui.handlers ne permet pas les doublons)", name),
                    ERR_TAURI_DUPLICATE_HANDLER,
                );
            }
        }
        w.handlers.insert(name, HandlerInfo { trampoline_addr, param_names });
    }
}

/// Recherche le trampoline enregistré pour `command_name` sur la fenêtre `this`.
/// Utilisé par l'invoke_handler installé dans Tauri_run (pas exposé à Ocara).
fn lookup_handler(this: i64, command_name: &str) -> Option<i64> {
    let map = TAURI_WINDOWS.lock().unwrap();
    let win = map.get(&this)?;
    let w = win.lock().unwrap();
    w.handlers.get(command_name).map(|h| h.trampoline_addr)
}

/// Construit, pour la fenêtre `this`, le shim JS à injecter au chargement de la
/// page (voir Tauri_run) : `{"cmd1":["a","b"],"cmd2":["x"]}` — une entrée par
/// handler enregistré, avec ses noms de paramètres dans l'ordre déclaré.
fn handler_param_names_map(this: i64) -> HashMap<String, Vec<String>> {
    let map = TAURI_WINDOWS.lock().unwrap();
    match map.get(&this) {
        Some(win) => win.lock().unwrap().handlers.iter()
            .map(|(name, info)| (name.clone(), info.param_names.clone()))
            .collect(),
        None => HashMap::new(),
    }
}

/// Construit le script d'initialisation à injecter dans la webview (voir Tauri_run,
/// via `.initialization_script()` — exécuté au document-start, avant tout script de
/// la page, y compris pour une URL externe).
///
/// Expose `window.ocara.invoke(cmd, payload, options)` : identique à
/// `window.__TAURI_INTERNALS__.invoke`, mais accepte AUSSI un payload *array*
/// positionnel (`window.ocara.invoke("cmd", [v1, v2])`) en le convertissant en
/// objet nommé (`{a: v1, b: v2}`) grâce aux noms de paramètres enregistrés par
/// ui.handler/ui.handlers, avant de déléguer au vrai pont IPC de Tauri.
///
/// Note : on ne peut PAS se contenter de redéfinir `window.__TAURI_INTERNALS__.invoke`
/// lui-même — Tauri le déclare via `Object.defineProperty` sans `configurable: true`
/// (scripts/core.js), donc toute tentative de le réécrire ensuite lève une
/// `TypeError: Cannot redefine property`. `window.ocara` est un espace de noms à
/// nous, sans ce verrou : c'est la façade officielle Ocara pour l'IPC JS→Ocara.
fn build_ipc_shim_script(this: i64) -> String {
    let param_names = handler_param_names_map(this);
    let names_json = serde_json::to_string(&param_names).unwrap_or_else(|_| "{}".to_string());
    format!(
        r#"(function() {{
  var __ocaraHandlerParams = {names_json};
  window.ocara = window.ocara || {{}};
  window.ocara.invoke = function(cmd, payload, options) {{
    if (Array.isArray(payload) && Object.prototype.hasOwnProperty.call(__ocaraHandlerParams, cmd)) {{
      var names = __ocaraHandlerParams[cmd];
      var obj = {{}};
      for (var i = 0; i < names.length; i++) {{ obj[names[i]] = payload[i]; }}
      payload = obj;
    }}
    return window.__TAURI_INTERNALS__.invoke(cmd, payload, options);
  }};
}})();
"#,
        names_json = names_json
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers JSON pour les trampolines générés par le compilateur.
//
// Convention des arguments : un unique payload JSON reçu de JS, sous forme
// d'OBJET nommé — `invoke("cmd", {a: v1, b: v2})` — chaque valeur est lue par
// nom de paramètre Ocara, pas par position. C'est la seule forme que le pont
// IPC natif de Tauri délivre fidèlement : un payload *array* top-level est
// intercepté par le JS interne de Tauri (scripts/process-ipc-message-fn.js,
// `Array.isArray(message)`) et traité comme du binaire brut (octet-stream)
// AVANT même d'atteindre ce code — jamais reçu comme JSON ici. La syntaxe
// `invoke("cmd", [v1, v2])` reste utilisable côté JS utilisateur grâce au shim
// injecté par tauri_ipc_shim_script, qui réécrit l'array en objet nommé (via
// les param_names enregistrés) avant que Tauri lui-même ne le voie.
//
// Chaque "get" suppose que l'appelant (le trampoline généré) a déjà vérifié
// via le "is" correspondant que la clé existe et a le bon type ; sinon la
// valeur retournée est une valeur sentinelle (0 / chaîne vide), jamais un crash.
// ─────────────────────────────────────────────────────────────────────────────

fn parse_args_object(json_ptr: i64) -> Option<serde_json::Map<String, serde_json::Value>> {
    let s = unsafe { ptr_to_str(json_ptr) };
    match serde_json::from_str::<serde_json::Value>(s).ok()? {
        serde_json::Value::Object(map) => Some(map),
        _ => None,
    }
}

fn get_key(json_ptr: i64, key_ptr: i64) -> Option<serde_json::Value> {
    let key = unsafe { ptr_to_str(key_ptr) };
    parse_args_object(json_ptr).and_then(|mut m| m.remove(key))
}

#[unsafe(no_mangle)]
pub extern "C" fn __tauri_obj_is_string(json_ptr: i64, key_ptr: i64) -> i64 {
    match get_key(json_ptr, key_ptr) {
        Some(serde_json::Value::String(_)) => 1,
        _ => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_obj_is_int(json_ptr: i64, key_ptr: i64) -> i64 {
    match get_key(json_ptr, key_ptr) {
        Some(serde_json::Value::Number(n)) => if n.is_i64() || n.is_u64() { 1 } else { 0 },
        _ => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_obj_is_float(json_ptr: i64, key_ptr: i64) -> i64 {
    match get_key(json_ptr, key_ptr) {
        Some(serde_json::Value::Number(n)) => if n.as_f64().is_some() { 1 } else { 0 },
        _ => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_obj_is_bool(json_ptr: i64, key_ptr: i64) -> i64 {
    match get_key(json_ptr, key_ptr) {
        Some(serde_json::Value::Bool(_)) => 1,
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __tauri_obj_get_string(json_ptr: i64, key_ptr: i64) -> i64 {
    let s = get_key(json_ptr, key_ptr)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    unsafe { alloc_str(&s) }
}
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_obj_get_int(json_ptr: i64, key_ptr: i64) -> i64 {
    get_key(json_ptr, key_ptr).and_then(|v| v.as_i64()).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_obj_get_float(json_ptr: i64, key_ptr: i64) -> f64 {
    get_key(json_ptr, key_ptr).and_then(|v| v.as_f64()).unwrap_or(0.0)
}
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_obj_get_bool(json_ptr: i64, key_ptr: i64) -> i64 {
    match get_key(json_ptr, key_ptr) {
        Some(serde_json::Value::Bool(b)) => if b { 1 } else { 0 },
        _ => 0,
    }
}

/// Construit l'enveloppe de réponse `{"ok":true,"value":<v>}` (v déjà une
/// valeur JSON encodée en string, ex: `"5"`, `"\"texte\""`, `"true"`).
fn ok_envelope(value_json: &str) -> i64 {
    let s = format!("{{\"ok\":true,\"value\":{}}}", value_json);
    unsafe { alloc_str(&s) }
}
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_ok_string(value_ptr: i64) -> i64 {
    let s = unsafe { ptr_to_str(value_ptr) };
    ok_envelope(&serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string()))
}
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_ok_int(value: i64) -> i64 { ok_envelope(&value.to_string()) }
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_ok_float(value: f64) -> i64 { ok_envelope(&value.to_string()) }
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_ok_bool(value: i64) -> i64 { ok_envelope(if value != 0 { "true" } else { "false" }) }
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_ok_void() -> i64 { ok_envelope("null") }

/// Construit l'enveloppe d'erreur `{"ok":false,"error":"<msg>"}`.
#[unsafe(no_mangle)]
pub extern "C" fn __tauri_err(msg_ptr: i64) -> i64 {
    let msg = unsafe { ptr_to_str(msg_ptr) };
    let s = format!("{{\"ok\":false,\"error\":{}}}", serde_json::to_string(msg).unwrap_or_else(|_| "\"\"".to_string()));
    unsafe { alloc_str(&s) }
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 1 : vraie fenêtre native (pont vers le crate `tauri`)
//
// Tout ce qui précède dans ce fichier est une simulation en mémoire (aucune
// fenêtre réelle). `Tauri_run` est le premier pont vers le vrai crate `tauri` :
// il construit un `tauri::Context` dynamiquement (pas de tauri.conf.json ni de
// macro `generate_context!()` — voir `tauri::Context::new`, un constructeur
// public) à partir de l'état actuellement stocké dans TAURI_WINDOWS, puis
// lance une vraie fenêtre GTK/WebKit.
//
// Portée de cette phase : une seule fenêtre déclarée statiquement via
// `config.tauri.windows`, pas encore de pont IPC listen/emit réel (toujours
// simulé), pas de dialog/notify réels. `run()` BLOQUE le thread appelant
// jusqu'à la fermeture de la fenêtre — c'est une contrainte du crate `tauri`,
// pas un choix : tout enregistrement (listen, etc.) doit se faire avant.
// ─────────────────────────────────────────────────────────────────────────────

/// Implémentation de `tauri::Assets<R>` qui lit les fichiers du frontend
/// directement sur le disque (au lieu de les embarquer à la compilation du
/// runtime, ce qui serait figé pour tous les programmes Ocara).
/// Base = répertoire courant du processus au moment de `use Tauri(...)`.
struct DiskAssets {
    base_dir: std::path::PathBuf,
}

impl<R: tauri::Runtime> tauri::Assets<R> for DiskAssets {
    fn get(&self, key: &tauri::utils::assets::AssetKey) -> Option<std::borrow::Cow<'_, [u8]>> {
        let rel = key.as_ref().trim_start_matches('/');
        let rel = if rel.is_empty() { "index.html" } else { rel };
        let full = self.base_dir.join(rel);
        std::fs::read(&full).ok().map(std::borrow::Cow::Owned)
    }

    fn iter(&self) -> Box<tauri::utils::assets::AssetsIter<'_>> {
        // Pas d'introspection nécessaire (pas de bundling) : liste vide.
        Box::new(std::iter::empty())
    }

    fn csp_hashes(
        &self,
        _html_path: &tauri::utils::assets::AssetKey,
    ) -> Box<dyn Iterator<Item = tauri::utils::assets::CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

/// Lance réellement la fenêtre Tauri configurée par `use Tauri(...)` et les
/// appels effectués avant `run()`. Bloque jusqu'à la fermeture de la fenêtre.
/// Retire les variables GTK que des applications GTK tierces (typiquement VS Code,
/// distribué en snap) injectent dans l'environnement hérité par ce process. GTK les
/// lit au premier `gtk_init` et tente de charger LEURS modules (ex: un plugin son
/// dans le sandbox snap de VS Code), dont les bibliothèques embarquées (libpthread
/// notamment) sont incompatibles avec la glibc du système : ça plante immédiatement
/// avec `undefined symbol: __libc_pthread_init` avant même qu'une fenêtre existe.
/// Sans rapport avec Ocara ou Tauri eux-mêmes — un artefact d'environnement de
/// terminal qu'on neutralise ici pour ne pas demander à chaque utilisateur de le
/// faire manuellement à chaque lancement.
fn clear_inherited_gtk_env() {
    for var in [
        "GTK_PATH",
        "GTK_EXE_PREFIX",
        "GDK_PIXBUF_MODULE_FILE",
        "GDK_PIXBUF_MODULEDIR",
        "GIO_MODULE_DIR",
        "GTK_IM_MODULE_FILE",
    ] {
        unsafe { std::env::remove_var(var); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_run(this: i64) {
    clear_inherited_gtk_env();
    let (title, width, height, url) = {
        let map = TAURI_WINDOWS.lock().unwrap();
        match map.get(&this) {
            Some(win) => {
                let w = win.lock().unwrap();
                (w.title.clone(), w.width, w.height, w.url.clone())
            }
            None => {
                eprintln!("[Tauri_run] fenêtre introuvable pour ce handle, abandon.");
                return;
            }
        }
    };

    // "url" pointe soit vers un fichier local servi par DiskAssets (ex: "index.html"),
    // soit vers un serveur déjà en cours d'exécution (ex: "http://localhost:8080") —
    // dans ce second cas c'est une vraie URL externe, pas un asset embarqué.
    let webview_url = match url::Url::parse(&url) {
        Ok(u) if u.scheme() == "http" || u.scheme() == "https" => {
            tauri::WebviewUrl::External(u)
        }
        _ => tauri::WebviewUrl::App(url.into()),
    };

    // La fenêtre n'est PAS déclarée ici via `config.app.windows` : on la construit
    // nous-mêmes plus bas (dans `.setup()`, via WebviewWindowBuilder) pour pouvoir
    // lui attacher `.initialization_script(...)` — le shim JS `window.ocara.invoke`
    // (voir build_ipc_shim_script). Une fenêtre déclarée en Config n'a aucun moyen
    // de recevoir de script d'initialisation supplémentaire.
    let mut config = tauri::utils::config::Config::default();
    config.identifier = "app.ocara.demo".to_string();

    let assets: Box<dyn tauri::Assets<tauri::Wry>> = Box::new(DiskAssets {
        base_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    });

    let package_info = tauri::PackageInfo {
        name: "Ocara App".to_string(),
        version: semver::Version::new(0, 1, 0),
        authors: "",
        description: "",
        crate_name: "ocara_app",
    };

    // Aucune commande IPC enregistrée pour l'instant (Phase 1 : juste la fenêtre) →
    // ACL résolue vide, ce qui suffit puisqu'on n'invoque encore aucune commande.
    // Tauri v2 rejette par défaut toute commande demandée par du contenu
    // "distant" (URL externe, y compris http://localhost — c'est notre cas,
    // puisqu'on pointe vers un vrai serveur Ocara, pas un asset embarqué) tant
    // qu'aucune permission explicite ne l'autorise. On construit ici une ACL
    // permissive mais ciblée : uniquement les commandes réellement enregistrées
    // via ui.handler/ui.handlers sur CETTE fenêtre, depuis n'importe quelle
    // origine http(s) — pas d'ouverture plus large que ça.
    let handler_names: Vec<String> = {
        let map = TAURI_WINDOWS.lock().unwrap();
        map.get(&this)
            .map(|win| win.lock().unwrap().handlers.keys().cloned().collect())
            .unwrap_or_default()
    };
    let any_origin = tauri::utils::acl::ExecutionContext::Remote {
        url: "http://*:*/*".parse().expect("motif ACL http://*:*/* invalide"),
    };
    let match_all = glob::Pattern::new("*").expect("motif glob * invalide");
    let mut allowed_commands: std::collections::BTreeMap<String, Vec<tauri::utils::acl::resolved::ResolvedCommand>> =
        std::collections::BTreeMap::new();
    for name in handler_names {
        allowed_commands.insert(name, vec![tauri::utils::acl::resolved::ResolvedCommand {
            context: any_origin.clone(),
            windows: vec![match_all.clone()],
            webviews: vec![match_all.clone()],
            scope_id: None,
            ..Default::default()
        }]);
    }
    let resolved_acl = tauri::utils::acl::resolved::Resolved {
        has_app_acl: false,
        allowed_commands,
        ..Default::default()
    };
    #[cfg(debug_assertions)]
    let authority = tauri::ipc::RuntimeAuthority::new(std::collections::BTreeMap::new(), resolved_acl);
    #[cfg(not(debug_assertions))]
    let authority = tauri::ipc::RuntimeAuthority::new(resolved_acl);

    let context = tauri::Context::new(
        config,
        assets,
        None, // default_window_icon
        None, // app_icon
        package_info,
        tauri::Pattern::Brownfield,
        authority,
        None, // plugin_global_api_scripts
    );

    // Un seul invoke_handler dynamique : il ne connaît aucune commande à la
    // compilation du runtime (elles sont définies dans le programme Ocara de
    // l'utilisateur), il se contente de chercher le nom demandé par JS dans le
    // registre de handlers de CETTE fenêtre (voir Tauri_handler_register) et
    // d'appeler le trampoline correspondant avec le JSON brut des arguments.
    let invoke_handler = move |invoke: tauri::ipc::Invoke<tauri::Wry>| -> bool {
        let command = invoke.message.command().to_string();
        let args_json: String = match invoke.message.payload() {
            tauri::ipc::InvokeBody::Json(v) => v.to_string(),
            // Un payload Raw (binaire) ne peut provenir que d'un array top-level envoyé
            // directement à window.__TAURI_INTERNALS__.invoke (voir la note dans
            // build_ipc_shim_script) : pas notre convention (objet nommé attendu), donc
            // pas de valeurs à en tirer — {} laisse simplement chaque clé manquante,
            // ce que le trampoline généré rejette proprement (message d'erreur clair).
            tauri::ipc::InvokeBody::Raw(_) => "{}".to_string(),
        };

        match lookup_handler(this, &command) {
            Some(trampoline_addr) => {
                let args_ptr = unsafe { alloc_str(&args_json) };
                let result_ptr = unsafe {
                    let f: unsafe extern "C" fn(i64) -> i64 = std::mem::transmute(trampoline_addr as usize);
                    f(args_ptr)
                };
                let result_str = unsafe { ptr_to_str(result_ptr) }.to_string();
                match serde_json::from_str::<serde_json::Value>(&result_str) {
                    Ok(serde_json::Value::Object(obj)) if obj.get("ok").and_then(|v| v.as_bool()) == Some(true) => {
                        invoke.resolver.resolve(obj.get("value").cloned().unwrap_or(serde_json::Value::Null));
                    }
                    Ok(serde_json::Value::Object(obj)) => {
                        let msg = obj.get("error").and_then(|v| v.as_str()).unwrap_or("erreur inconnue").to_string();
                        invoke.resolver.reject(msg);
                    }
                    _ => invoke.resolver.reject("réponse invalide du handler Ocara".to_string()),
                }
            }
            None => {
                invoke.resolver.reject(format!("aucun handler enregistré pour la commande '{}'", command));
            }
        }
        true
    };

    // Construit la fenêtre elle-même (titre/taille/URL déjà résolus plus haut) avec
    // son script d'initialisation — exécuté par le moteur de webview au tout début
    // du chargement de CHAQUE page (y compris une URL externe), avant n'importe quel
    // script de la page elle-même. C'est le seul point d'accroche fiable pour exposer
    // `window.ocara.invoke` : voir build_ipc_shim_script pour le détail.
    let shim_script = build_ipc_shim_script(this);
    let setup = move |app: &mut tauri::App<tauri::Wry>| -> Result<(), Box<dyn std::error::Error>> {
        tauri::WebviewWindowBuilder::new(app, "main", webview_url)
            .title(&title)
            .inner_size(width as f64, height as f64)
            .initialization_script(&shim_script)
            .build()?;
        Ok(())
    };

    if let Err(e) = tauri::Builder::<tauri::Wry>::new()
        .invoke_handler(invoke_handler)
        .setup(setup)
        .run(context) {
        eprintln!("[Tauri_run] erreur au lancement : {e}");
    }
}
