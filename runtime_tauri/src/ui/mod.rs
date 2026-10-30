// Backend desktop d'`ocara.UI` : un objet `UI` est une fenêtre Tauri (même
// état, même finaliseur, mêmes boîtes de dialogue que `ocara.Tauri`) dont la
// page est le moteur de rendu (effects.js, engine.js) affichant l'arbre du `.oc.ui`.
// État, actions et thread des gestionnaires : crate `runtime_ui`.

use serde_json::{json, Value};

use ocara_runtime_ui as core;
use ocara_runtime_ui::ffi::str_arg;

use crate::window::live;

#[cfg(target_os = "linux")]
mod webview;
#[cfg(not(target_os = "linux"))]
#[path = "webview_stub.rs"]
mod webview;

mod reload;

pub(crate) use webview::release as release_webviews;

/// Page du moteur, servie par `DiskAssets` (app.rs).
pub(crate) const PAGE: &str = "__ocara_ui.html";

/// Commande IPC des messages du moteur.
pub(crate) const EVENT_COMMAND: &str = "__ocara_ui_event";

const ENGINE_JS: &str = concat!(include_str!("effects.js"), include_str!("engine.js"));
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
    if message.get("webview").is_some() {
        return webview::place(this, &message);
    }
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
pub extern "C" fn UI_init(this: i64, tree_ptr: i64, vars: i64) {
    unsafe { crate::__rc_register_window_finalizer(crate::finalize_window) };
    let tree = core::init(this, &str_arg(tree_ptr), ocara_runtime_ui::ffi::to_json(vars), default_action, send_set);
    let window = tree.get("window").cloned().unwrap_or(json!({}));
    let mut options = window.as_object().cloned().unwrap_or_default();
    if let Some(Value::Object(tauri)) = options.remove("tauri") {
        options.extend(tauri);
    }
    options.remove("android");
    crate::register_window(this, &Value::Object(options), Some(tree.to_string()));
}

/// Nouvel arbre affiché (variables, rechargement) : gardé pour une page
/// rechargée ou une fenêtre ouverte plus tard, et remis au moteur.
pub(crate) fn show_tree(this: i64, tree: &Value) {
    crate::window::state(this, |w| w.ui_tree = Some(tree.to_string()));
    eval(this, &format!("OcaraUI.reload({tree})"));
}

/// `ui.setVars(vars)` : textes remplis à nouveau (changement de langue).
#[unsafe(no_mangle)]
pub extern "C" fn UI_setVars(this: i64, vars: i64) {
    let tree = core::set_vars(this, ocara_runtime_ui::ffi::to_json(vars));
    show_tree(this, &tree);
}

/// `ui.sizeClass()` → `string` : classe de taille de la fenêtre.
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
    eval(this, &format!("OcaraUI.set({}, {}, {value})", json!(id), json!(prop)));
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_set(this: i64, id: i64, prop: i64, value: i64) {
    let (id, prop) = (str_arg(id), str_arg(prop));
    let value = core::set(this, &id, &prop, value);
    send_set(this, &id, &prop, &value);
    webview::apply(this, &id, &prop);
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

/// `ui.animate(id, name, options)` : animation déclarée sous `animations:`.
#[unsafe(no_mangle)]
pub extern "C" fn UI_animate(this: i64, id: i64, name: i64, options: i64) {
    let (id, name) = (str_arg(id), str_arg(name));
    let options = core::animate(this, &id, &name, ocara_runtime_ui::ffi::to_json(options));
    eval(this, &format!("OcaraUI.animate({}, {}, {options})", json!(id), json!(name)));
}

/// `ui.animate(id, name)` : options du `.oc.ui`.
#[unsafe(no_mangle)]
pub extern "C" fn UI_animate_2(this: i64, id: i64, name: i64) {
    UI_animate(this, id, name, 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_stopAnimation(this: i64, id: i64) {
    let id = str_arg(id);
    core::stop_animation(this, &id);
    eval(this, &format!("OcaraUI.stopAnimation({})", json!(id)));
}

/// `ui.validate(container)` → `array<string>` : champs obligatoires vides.
#[unsafe(no_mangle)]
pub extern "C" fn UI_validate(this: i64, container: i64) -> i64 {
    ocara_runtime_ui::ffi::from_json(&serde_json::Value::from(core::validate(this, &str_arg(container))))
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
    eval(this, &format!("OcaraUI.toast({})", json!(core::text(this, &str_arg(text)))));
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_run(this: i64) {
    // Binaire de test ocaraunit, thread principal : rend la main (signalé).
    if ocara_runtime::blocking::neutralized("UI.run") {
        return;
    }
    reload::watch(this);
    crate::app::Tauri_run(this);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_close(this: i64) {
    crate::window::Tauri_close(this);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_alert(this: i64, title: i64, text: i64) {
    let (title, text) = (message(this, title), message(this, text));
    crate::dialogs::Tauri_alert(this, title.0, text.0);
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_confirm(this: i64, title: i64, text: i64) -> i64 {
    let (title, text) = (message(this, title), message(this, text));
    crate::dialogs::Tauri_confirm(this, title.0, text.0)
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
    let (title, text) = (message(this, title), message(this, text));
    crate::dialogs::Tauri_notify(this, title.0, text.0);
}

/// Texte Ocara d'un message, variables `${nom}` remplies ; libéré avec la valeur.
struct Message(i64);

impl Drop for Message {
    fn drop(&mut self) {
        ocara_runtime_ui::ffi::release(self.0);
    }
}

fn message(this: i64, text: i64) -> Message {
    Message(ocara_runtime_ui::ffi::string(&core::text(this, &str_arg(text))))
}

/// `ui.postMessage(id, data)` : message à la page d'un composant `webview`.
#[unsafe(no_mangle)]
pub extern "C" fn UI_postMessage(this: i64, id: i64, data: i64) {
    webview::post_message(this, &str_arg(id), &ocara_runtime_ui::ffi::to_json(data));
}

/// `ui.eval(id, script)` → `string` : résultat du script dans la page.
#[unsafe(no_mangle)]
pub extern "C" fn UI_eval(this: i64, id: i64, script: i64) -> i64 {
    ocara_runtime_ui::ffi::string(&webview::eval(this, &str_arg(id), &str_arg(script)))
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_webBack(this: i64, id: i64) {
    webview::navigate(this, &str_arg(id), "back");
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_webForward(this: i64, id: i64) {
    webview::navigate(this, &str_arg(id), "forward");
}

#[unsafe(no_mangle)]
pub extern "C" fn UI_reload(this: i64, id: i64) {
    webview::navigate(this, &str_arg(id), "reload");
}

/// `ui.tauri()` → `Tauri` : l'objet `UI` est lui-même la fenêtre Tauri ;
/// référence supplémentaire rendue à l'appelant.
#[unsafe(no_mangle)]
pub extern "C" fn UI_tauri(this: i64) -> i64 {
    ocara_runtime::rc::__rc_retain(this);
    this
}
