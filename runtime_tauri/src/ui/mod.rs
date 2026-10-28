// Backend desktop d'`ocara.UI` : un objet `UI` est une fenêtre Tauri (même
// état, même finaliseur, mêmes boîtes de dialogue que `ocara.Tauri`) dont la
// page est le moteur de rendu (engine.js) affichant l'arbre du `.oc.ui`.
// État, actions et thread des gestionnaires : crate `runtime_ui`.

use serde_json::{json, Value};

use ocara_runtime_ui as core;
use ocara_runtime_ui::ffi::str_arg;

use crate::window::live;

/// Page du moteur, servie par `DiskAssets` (app.rs).
pub(crate) const PAGE: &str = "__ocara_ui.html";

/// Commande IPC des messages du moteur.
pub(crate) const EVENT_COMMAND: &str = "__ocara_ui_event";

const ENGINE_JS: &str = include_str!("engine.js");
const ENGINE_CSS: &str = include_str!("engine.css");

pub(crate) fn page() -> String {
    format!(
        "<!doctype html>\n<html lang=\"fr\">\n<head>\n<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<style>\n{ENGINE_CSS}\n</style>\n</head>\n\
<body>\n<div id=\"oc-root\"></div>\n<script>\n{ENGINE_JS}\n</script>\n<script>\n\
OcaraUI.start(window.__OCARA_UI_TREE, {{ platform: \"tauri\", send: function (m) {{\n\
  window.__TAURI_INTERNALS__.invoke(\"{EVENT_COMMAND}\", {{ message: JSON.stringify(m) }});\n\
}} }});\n</script>\n</body>\n</html>\n"
    )
}

/// Script d'initialisation de la fenêtre : l'arbre, lu par la page.
pub(crate) fn boot_script(tree: &str) -> String {
    format!("window.__OCARA_UI_TREE = {tree};")
}

/// Message du moteur (`{ message: "<json>" }`). À `ready` (page chargée ou
/// rechargée), l'état gardé est rendu à la page.
pub(crate) fn receive(this: i64, args_json: &str) {
    let args: Value = serde_json::from_str(args_json).unwrap_or(Value::Null);
    let message: Value = args.get("message").and_then(Value::as_str)
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or(Value::Null);
    if message.get("action").and_then(Value::as_str) == Some("ready") {
        eval(this, &format!("OcaraUI.restore({})", core::state(this)));
    }
    core::post(this, &message);
}

fn eval(this: i64, script: &str) {
    if let Some(window) = live(this) {
        let _ = window.eval(script);
    }
}

/// Action sans gestionnaire : `close` ferme la fenêtre.
fn default_action(this: i64, action: &str) {
    if action == "close" {
        crate::window::Tauri_close(this);
    }
}

/// `use UI(arbre)` : l'arbre normalisé, mis en littéral par le compilateur
/// (src/core/ui_file.rs).
#[unsafe(no_mangle)]
pub extern "C" fn UI_init(this: i64, tree_ptr: i64) {
    unsafe { crate::__rc_register_window_finalizer(crate::finalize_window) };
    let tree_json = str_arg(tree_ptr);
    let tree = core::init(this, &tree_json, default_action);
    let window = tree.get("window").cloned().unwrap_or(json!({}));
    let mut options = window.as_object().cloned().unwrap_or_default();
    if let Some(Value::Object(tauri)) = options.remove("tauri") {
        options.extend(tauri);
    }
    options.remove("android");
    crate::register_window(this, &Value::Object(options), Some(tree_json));
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
    eval(this, &format!("OcaraUI.set({}, {}, {value})", json!(id), json!(prop)));
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_set(this: i64, id: i64, prop: i64, value: i64) {
    let (id, prop) = (str_arg(id), str_arg(prop));
    let value = core::set(this, &id, &prop, value);
    send_set(this, &id, &prop, &value);
}

/// `ui.setItems(id, rows)` : éléments d'une `list`.
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
    eval(this, &format!("OcaraUI.navigate({})", json!(screen)));
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_back(this: i64) {
    eval(this, "OcaraUI.back()");
}

/// `ui.toast(text)` : message bref en bas de la fenêtre.
#[unsafe(no_mangle)]
pub extern "C" fn UI_toast(this: i64, text: i64) {
    eval(this, &format!("OcaraUI.toast({})", json!(str_arg(text))));
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_run(this: i64) {
    crate::app::Tauri_run(this);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_close(this: i64) {
    crate::window::Tauri_close(this);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_alert(this: i64, title: i64, text: i64) {
    crate::dialogs::Tauri_alert(this, title, text);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_confirm(this: i64, title: i64, text: i64) -> i64 {
    crate::dialogs::Tauri_confirm(this, title, text)
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_pickFile(this: i64, extensions: i64) -> i64 {
    crate::dialogs::Tauri_pickFile(this, extensions)
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_saveFile(this: i64, name: i64) -> i64 {
    crate::dialogs::Tauri_saveFile(this, name)
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_notify(this: i64, title: i64, text: i64) {
    crate::dialogs::Tauri_notify(this, title, text);
}
