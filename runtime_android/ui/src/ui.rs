//! Backend Android d'`ocara.UI` : l'arbre du `.oc.ui` est affiché par le
//! moteur Compose (`OcaraUIEngine.kt`) dans l'Activity ; ses messages
//! reviennent par `OcaraBridge.nativeEvent("__ui", …)` (events.rs). État,
//! actions et thread des gestionnaires : crate `runtime_ui`. Une seule
//! interface est affichée à la fois.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Condvar, Mutex};

use serde_json::Value;

use ocara_runtime_android_common::java::{invoke, Arg, Ret};
use ocara_runtime_ui as core;
use ocara_runtime_ui::ffi::{raise, str_arg};

const SOURCE: &str = "UI";
const ENGINE: &str = "com.ocara.bridge.OcaraUIEngine";
const PLATFORM: &str = "com.ocara.bridge.OcaraUI";

unsafe extern "C" {
    /// runtime/src/rc.rs : finaliseur des objets `UI` (masque `W`).
    fn __rc_register_window_finalizer(finalizer: extern "C" fn(i64));
}

/// Interface affichée (destinataire des messages du moteur).
static SHOWN: AtomicI64 = AtomicI64::new(0);
/// Vrai tant que `run()` attend la fermeture.
static RUNNING: Mutex<bool> = Mutex::new(false);
static CLOSED: Condvar = Condvar::new();

fn engine(method: &str, args: &[Arg]) {
    invoke(SOURCE, ENGINE, method, args, Ret::Void);
}

extern "C" fn finalize(this: i64) {
    core::release(this);
    let _ = SHOWN.compare_exchange(this, 0, Ordering::SeqCst, Ordering::SeqCst);
}

/// Message du moteur (`nativeEvent("__ui", json)`), thread des événements.
pub(crate) fn receive(data: &str) {
    let this = SHOWN.load(Ordering::SeqCst);
    if this != 0 {
        let message: Value = serde_json::from_str(data).unwrap_or(Value::Null);
        core::post(this, &message);
    }
}

/// Action sans gestionnaire : `close`, et `back` sur l'écran de départ,
/// ferment l'app (comportement attendu du bouton retour).
fn default_action(this: i64, action: &str) {
    if matches!(action, "close" | "back") {
        UI_close(this);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_init(this: i64, tree: i64, vars: i64) {
    unsafe { __rc_register_window_finalizer(finalize) };
    crate::events::start();
    core::init(this, &str_arg(tree), ocara_runtime_ui::ffi::to_json(vars), default_action, send_set);
}

/// `ui.setVars(vars)` : textes remplis à nouveau (changement de langue).
#[unsafe(no_mangle)]
pub extern "C" fn UI_setVars(this: i64, vars: i64) {
    let tree = core::set_vars(this, ocara_runtime_ui::ffi::to_json(vars));
    if SHOWN.load(Ordering::SeqCst) == this {
        engine("reload", &[Arg::Str(&tree.to_string())]);
    }
}

/// `ui.sizeClass()` → `string` : classe de taille de l'écran.
#[unsafe(no_mangle)]
pub extern "C" fn UI_sizeClass(this: i64) -> i64 {
    ocara_runtime_ui::ffi::string(&core::size_class(this))
}

/// `ui.has(id)` → `bool` : nœud présent dans la variante affichée.
#[unsafe(no_mangle)]
pub extern "C" fn UI_has(this: i64, id: i64) -> i64 {
    core::has(this, &str_arg(id)) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_on(this: i64, action: i64, handler: i64) {
    core::on(this, &str_arg(action), handler);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_get(this: i64, id: i64, prop: i64) -> i64 {
    core::get(this, &str_arg(id), &str_arg(prop))
}

fn send_set(this: i64, id: &str, prop: &str, value: &Value) {
    if SHOWN.load(Ordering::SeqCst) == this {
        engine("set", &[Arg::Str(id), Arg::Str(prop), Arg::Str(&value.to_string())]);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_set(this: i64, id: i64, prop: i64, value: i64) {
    let (id, prop) = (str_arg(id), str_arg(prop));
    let value = core::set(this, &id, &prop, value);
    send_set(this, &id, &prop, &value);
}

/// `ui.bind(root, objet)` : chemins et accesseur générés par le compilateur
/// (runtime_ui/src/bind.rs).
#[unsafe(no_mangle)]
pub extern "C" fn UI_bind(this: i64, root: i64, object: i64, paths: i64, accessor: i64) {
    core::bind(this, &str_arg(root), object, &str_arg(paths), accessor);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_unbind(this: i64, root: i64) {
    core::unbind(this, &str_arg(root));
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_refresh(this: i64) {
    core::refresh(this);
}

/// `ui.validate(container)` → `array<string>` : champs obligatoires vides.
#[unsafe(no_mangle)]
pub extern "C" fn UI_validate(this: i64, container: i64) -> i64 {
    ocara_runtime_ui::ffi::from_json(&serde_json::Value::from(core::validate(this, &str_arg(container))))
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_setItems(this: i64, id: i64, rows: i64) {
    let id = str_arg(id);
    let value = core::set(this, &id, "items", rows);
    send_set(this, &id, "items", &value);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_navigate(this: i64, screen: i64) {
    let screen = str_arg(screen);
    core::check_screen(this, &screen);
    if SHOWN.load(Ordering::SeqCst) == this {
        engine("navigate", &[Arg::Str(&screen)]);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_back(this: i64) {
    if SHOWN.load(Ordering::SeqCst) == this {
        engine("goBack", &[]);
    }
}

/// `ui.run()` : affiche l'interface (état posé avant compris) et attend sa
/// fermeture (`ui.close()`, action `close`, retour sur l'écran de départ).
#[unsafe(no_mangle)]
pub extern "C" fn UI_run(this: i64) {
    let tree = core::tree(this).unwrap_or(Value::Null);
    SHOWN.store(this, Ordering::SeqCst);
    *RUNNING.lock().unwrap() = true;
    engine("show", &[Arg::Str(&tree.to_string()), Arg::Str(&core::state(this).to_string())]);
    let mut running = RUNNING.lock().unwrap();
    while *running {
        running = CLOSED.wait(running).unwrap();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_close(_this: i64) {
    invoke(SOURCE, PLATFORM, "close", &[], Ret::Void);
    *RUNNING.lock().unwrap() = false;
    CLOSED.notify_all();
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_toast(this: i64, text: i64) {
    invoke(SOURCE, PLATFORM, "toast", &[Arg::Str(&core::text(this, &str_arg(text)))], Ret::Void);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_alert(this: i64, title: i64, text: i64) {
    let (title, text) = (core::text(this, &str_arg(title)), core::text(this, &str_arg(text)));
    invoke(SOURCE, PLATFORM, "alert", &[Arg::Str(&title), Arg::Str(&text)], Ret::Void);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_confirm(this: i64, title: i64, text: i64) -> i64 {
    let (title, text) = (core::text(this, &str_arg(title)), core::text(this, &str_arg(text)));
    invoke(SOURCE, PLATFORM, "confirm", &[Arg::Str(&title), Arg::Str(&text)], Ret::Bool).bool()
}

/// `ui.pickFile(extensions)` : sélecteur du système ; le document est copié
/// dans le cache de l'app (chemin rendu), `""` si annulé.
#[unsafe(no_mangle)]
pub extern "C" fn UI_pickFile(_this: i64, extensions: i64) -> i64 {
    let wanted = match ocara_runtime_ui::ffi::to_json(extensions) {
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(","),
        _ => String::new(),
    };
    let path = invoke(SOURCE, "com.ocara.bridge.OcaraFilePicker", "pickFile", &[Arg::Str(&wanted)], Ret::Str);
    let path = match path {
        ocara_runtime_android_common::java::Value::Str(path) => path,
        _ => String::new(),
    };
    ocara_runtime_ui::ffi::string(&path)
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_saveFile(_this: i64, _name: i64) -> i64 {
    raise(core::ERR_UNAVAILABLE, "saveFile : pas de chemin de fichier sur Android (AndroidFiles::saveToDownloads)".to_string())
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_notify(this: i64, title: i64, text: i64) {
    let (title, text) = (core::text(this, &str_arg(title)), core::text(this, &str_arg(text)));
    invoke(SOURCE, "com.ocara.bridge.OcaraNotifications", "show", &[Arg::Str(&title), Arg::Str(&text)], Ret::Int);
}

/// `ui.tauri()` : pas de fenêtre Tauri sur Android (spécificités : `AndroidUI::…`).
#[unsafe(no_mangle)]
pub extern "C" fn UI_tauri(_this: i64) -> i64 {
    raise(core::ERR_UNAVAILABLE, "ui.tauri() : desktop seulement (sur Android : AndroidUI::…)".to_string())
}

const WEBVIEWS: &str = "com.ocara.bridge.OcaraUIWebViews";

/// `ui.postMessage(id, data)` : message à la page d'un composant `webview`.
#[unsafe(no_mangle)]
pub extern "C" fn UI_postMessage(_this: i64, id: i64, data: i64) {
    let data = ocara_runtime_ui::ffi::to_json(data).to_string();
    invoke(SOURCE, WEBVIEWS, "postMessage", &[Arg::Str(&str_arg(id)), Arg::Str(&data)], Ret::Void);
}

/// `ui.eval(id, script)` → `string`.
#[unsafe(no_mangle)]
pub extern "C" fn UI_eval(_this: i64, id: i64, script: i64) -> i64 {
    let text = invoke(SOURCE, WEBVIEWS, "eval", &[Arg::Str(&str_arg(id)), Arg::Str(&str_arg(script))], Ret::Str);
    let text = match text {
        ocara_runtime_android_common::java::Value::Str(text) => text,
        _ => String::new(),
    };
    ocara_runtime_ui::ffi::string(&text)
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_webBack(_this: i64, id: i64) {
    invoke(SOURCE, WEBVIEWS, "goBack", &[Arg::Str(&str_arg(id))], Ret::Void);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_webForward(_this: i64, id: i64) {
    invoke(SOURCE, WEBVIEWS, "goForward", &[Arg::Str(&str_arg(id))], Ret::Void);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_reload(_this: i64, id: i64) {
    invoke(SOURCE, WEBVIEWS, "reload", &[Arg::Str(&str_arg(id))], Ret::Void);
}
