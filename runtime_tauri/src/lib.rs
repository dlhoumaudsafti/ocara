
// runtime_tauri/src/lib.rs — intégration Tauri native, crate séparé de
// ocara_runtime (voir la doc dans runtime_tauri/Cargo.toml et
// src/codegen/link.rs : c'est ce qui permet de ne lier GTK/WebKit que pour les
// programmes qui importent réellement ocara.Tauri).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

use ocara_runtime::{alloc_str, ptr_to_str};

mod app;
mod dialogs;
mod events;
mod menus;
mod shortcuts;
mod tray;
mod ui;
mod window;

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
pub(crate) struct OcaraTauriWindow {
    title: String,
    width: i64,
    height: i64,
    url: String,
    is_open: bool,
    is_minimized: bool,
    is_maximized: bool,
    has_focus: bool,
    /// Callbacks `ui.listen` par événement (retenus, voir events.rs).
    event_callbacks: HashMap<String, Vec<i64>>,
    // Handlers JS→Ocara enregistrés via ui.handler(name, fn) / ui.handlers({...}).
    // Contrairement à event_callbacks (fire-and-forget), la valeur ici est
    // l'adresse d'un trampoline *généré par le compilateur* pour CE couple
    // (nom, fonction) précis : il connaît statiquement la signature réelle de
    // la fonction Ocara ciblée, décode les arguments JSON en conséquence, puis
    // appelle la vraie fonction et sérialise son retour. Signature uniforme :
    // extern "C" fn(json_args_ptr: i64) -> i64 (pointeur vers une string JSON
    // "{"ok":true,"value":...}" ou "{"ok":false,"error":"..."}").
    handlers: HashMap<String, HandlerInfo>, // command_name → (trampoline_ptr, noms de paramètres ordonnés)
    /// Application Tauri pendant `run()` : les méthodes de fenêtre agissent
    /// alors sur la vraie fenêtre (voir window.rs).
    app: Option<tauri::AppHandle>,
    /// Barre de menus et menu contextuel décrits par `setMenu` /
    /// `setContextMenu` (JSON), entrées cochables construites (menus.rs).
    menu_spec: Option<serde_json::Value>,
    context_spec: Option<serde_json::Value>,
    menu_checks: Vec<tauri::menu::CheckMenuItem<tauri::Wry>>,
    context_checks: Vec<tauri::menu::CheckMenuItem<tauri::Wry>>,
    context_menu: Option<tauri::menu::Menu<tauri::Wry>>,
    /// Inspecteur web autorisé (`setDevTools`, appliqué à la création).
    devtools: bool,
    /// Étiquette de la vraie fenêtre (`main` pour le premier `run()`), vide
    /// tant qu'elle n'est pas créée.
    label: String,
    /// Options de `use Tauri` (`decorations`, `alwaysOnTop`, `transparent`,
    /// `resizable`, `minWidth`/`minHeight`), réglables ensuite.
    decorations: bool,
    always_on_top: bool,
    transparent: bool,
    resizable: bool,
    center: bool,
    min_size: Option<(i64, i64)>,
    /// Icône de zone de notification (tray.rs) et raccourcis globaux
    /// (shortcuts.rs, `(raccourci, action)`).
    tray_spec: Option<serde_json::Value>,
    tray: Option<tauri::tray::TrayIcon>,
    tray_checks: Vec<tauri::menu::CheckMenuItem<tauri::Wry>>,
    shortcuts: Vec<(String, String)>,
    /// Objet `UI` : arbre normalisé du `.oc.ui`, affiché par le moteur
    /// (ui/mod.rs) au lieu d'une page à `url`.
    ui_tree: Option<String>,
}

/// Réglages de construction d'une fenêtre (app.rs, `build_window`).
pub(crate) struct WindowConfig {
    pub(crate) title: String,
    pub(crate) width: i64,
    pub(crate) height: i64,
    pub(crate) url: String,
    pub(crate) maximized: bool,
    pub(crate) minimized: bool,
    pub(crate) devtools: bool,
    pub(crate) decorations: bool,
    pub(crate) always_on_top: bool,
    pub(crate) transparent: bool,
    pub(crate) resizable: bool,
    pub(crate) center: bool,
    pub(crate) min_size: Option<(i64, i64)>,
    pub(crate) ui_tree: Option<String>,
}

impl OcaraTauriWindow {
    pub(crate) fn clone_config(&self) -> WindowConfig {
        WindowConfig {
            title: self.title.clone(),
            width: self.width,
            height: self.height,
            url: self.url.clone(),
            maximized: self.is_maximized,
            minimized: self.is_minimized,
            devtools: self.devtools,
            decorations: self.decorations,
            always_on_top: self.always_on_top,
            transparent: self.transparent,
            resizable: self.resizable,
            center: self.center,
            min_size: self.min_size,
            ui_tree: self.ui_tree.clone(),
        }
    }
}

// Table globale des fenêtres créées (clé = pointeur Ocara)
// Convention du projet : once_cell::sync::Lazy plutôt que lazy_static (cf. dotenv.rs)
pub(crate) static TAURI_WINDOWS: Lazy<Mutex<HashMap<i64, Arc<Mutex<OcaraTauriWindow>>>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

/// Options de `use Tauri` (`map<string, mixed>`) en JSON : les booléens
/// et nombres y sont lus avec leur type.
pub(crate) fn options_of(options_ptr: i64) -> serde_json::Value {
    let encoded = ocara_runtime::JSON_encode(options_ptr, 0);
    let options = serde_json::from_str(unsafe { ptr_to_str(encoded) }).unwrap_or(serde_json::Value::Null);
    ocara_runtime::rc::__rc_release(encoded);
    options
}

unsafe extern "C" {
    /// runtime/src/rc.rs : finaliseur appelé à la libération d'un objet `Tauri`.
    pub(crate) fn __rc_register_window_finalizer(finalizer: extern "C" fn(i64));
}

/// Libération de l'objet `Tauri` : son état et les callbacks `listen` retenus.
pub(crate) extern "C" fn finalize_window(this: i64) {
    tray::remove(this);
    ui::release_webviews(this);
    ocara_runtime_ui::release(this);
    let closed = TAURI_WINDOWS.lock().unwrap().remove(&this);
    if let Some(win) = closed {
        events::release_listeners(&win.lock().unwrap().event_callbacks);
    }
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
    unsafe { __rc_register_window_finalizer(finalize_window) };
    register_window(this, &options_of(options_ptr), None);
}

/// État d'une fenêtre d'après ses options (JSON de `use Tauri`, ou fenêtre
/// d'un `.oc.ui`, alors `ui_tree` : arbre affiché par le moteur d'`ocara.UI`).
pub(crate) fn register_window(this: i64, options: &serde_json::Value, ui_tree: Option<String>) {
    let text = |key: &str, default: &str| options.get(key).and_then(|v| v.as_str()).unwrap_or(default).to_string();
    let int = |key: &str| options.get(key).and_then(|v| v.as_i64());
    let flag = |key: &str, default: bool| options.get(key).and_then(|v| v.as_bool()).unwrap_or(default);
    let win = Arc::new(Mutex::new(OcaraTauriWindow {
        title: text("title", "Ocara App"),
        width: int("width").unwrap_or(800),
        height: int("height").unwrap_or(600),
        url: if ui_tree.is_some() { ui::PAGE.to_string() } else { text("url", "index.html") },
        is_open: true,
        is_minimized: false,
        is_maximized: false,
        has_focus: true,
        event_callbacks: HashMap::new(),
        handlers: HashMap::new(),
        app: None,
        menu_spec: None,
        context_spec: None,
        menu_checks: Vec::new(),
        context_checks: Vec::new(),
        context_menu: None,
        devtools: flag("devtools", false),
        label: String::new(),
        decorations: flag("decorations", true),
        always_on_top: flag("alwaysOnTop", false),
        transparent: flag("transparent", false),
        resizable: flag("resizable", true),
        center: flag("center", false),
        min_size: int("minWidth").zip(int("minHeight")),
        tray_spec: None,
        tray: None,
        tray_checks: Vec::new(),
        shortcuts: Vec::new(),
        ui_tree,
    }));
    TAURI_WINDOWS.lock().unwrap().insert(this, win);
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
                ocara_runtime::exception::throw_tauri_exception(
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
pub(crate) fn lookup_handler(this: i64, command_name: &str) -> Option<i64> {
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
pub(crate) fn build_ipc_shim_script(this: i64) -> String {
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
{events}"#,
        names_json = names_json,
        events = [events::EVENTS_SHIM, menus::MENU_SHIM].concat(),
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
