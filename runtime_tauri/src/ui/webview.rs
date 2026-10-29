// Composant `webview` d'ocara.UI sur desktop (Linux) : une vraie vue WebKit
// par nœud, isolée de la page du moteur (pas d'`<iframe>`), posée par-dessus
// elle dans un `gtk::Overlay` au rectangle que le moteur rapporte pour le nœud
// (`{ webview, x, y, width, height, visible }`). Tauri empile ses vues
// enfants dans la boîte de la fenêtre sous GTK, sans position : d'où WebKit
// directement. Tout passe par le thread principal GTK.
//
// Page → Ocara : `window.ocara.post(data)` (action `onMessage`).
// Ocara → page : `ui.postMessage(id, data)` (`window.ocara.onMessage(cb)`),
// `ui.eval(id, script)`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc;

use gtk::prelude::*;
use javascriptcore::ValueExt;
use serde_json::{json, Value};
use webkit2gtk::{
    LoadEvent, NavigationPolicyDecision, NavigationPolicyDecisionExt, PolicyDecisionExt, PolicyDecisionType,
    SettingsExt, URIRequestExt, UserContentInjectedFrames, UserContentManager, UserContentManagerExt, UserScript,
    UserScriptInjectionTime, WebView, WebViewExt,
};

use ocara_runtime_ui as core;

use crate::window::live;

/// Pont injecté dans chaque page au chargement.
const SHIM: &str = r#"(function () {
  window.ocara = window.ocara || {};
  var listeners = [];
  window.ocara.post = function (data) {
    window.webkit.messageHandlers.ocara.postMessage(JSON.stringify(data === undefined ? null : data));
  };
  window.ocara.onMessage = function (callback) { listeners.push(callback); };
  window.ocara.__receive = function (data) {
    listeners.slice().forEach(function (callback) {
      try { callback(data); } catch (e) { console.error(e); }
    });
  };
})();"#;

thread_local! {
    /// Vues par (objet UI, id) et calque de chaque fenêtre ; thread GTK seulement.
    static VIEWS: RefCell<HashMap<(i64, String), WebView>> = RefCell::new(HashMap::new());
    static OVERLAYS: RefCell<HashMap<i64, gtk::Overlay>> = RefCell::new(HashMap::new());
}

fn view(this: i64, id: &str) -> Option<WebView> {
    VIEWS.with(|views| views.borrow().get(&(this, id.to_string())).cloned())
}

/// Exécute `f` sur le thread GTK de la fenêtre de `this`.
fn on_main(this: i64, f: impl FnOnce(tauri::WebviewWindow) + Send + 'static) {
    if let Some(window) = live(this) {
        let target = window.clone();
        let _ = window.run_on_main_thread(move || f(target));
    }
}

/// Calque qui enveloppe la page du moteur (créé au premier composant).
fn overlay(this: i64, window: &tauri::WebviewWindow) -> Option<gtk::Overlay> {
    if let Some(overlay) = OVERLAYS.with(|o| o.borrow().get(&this).cloned()) {
        return Some(overlay);
    }
    let vbox = window.default_vbox().ok()?;
    let children = vbox.children();
    let index = children.iter().position(|w| w.type_().name() == "WebKitWebView")?;
    let engine = children[index].clone();
    let overlay = gtk::Overlay::new();
    vbox.remove(&engine);
    // Tauri et wry attachent à la page des gestionnaires de clic et de
    // toucher qui exigent `page → boîte → fenêtre` (redimensionnement d'une
    // fenêtre sans bordure, boutons précédent/suivant de la souris) : le
    // calque intercalé les ferait paniquer au premier clic.
    disconnect_all(&engine, "button-press-event");
    disconnect_all(&engine, "touch-event");
    overlay.add(&engine);
    vbox.pack_start(&overlay, true, true, 0);
    vbox.reorder_child(&overlay, index as i32);
    overlay.show_all();
    OVERLAYS.with(|o| o.borrow_mut().insert(this, overlay.clone()));
    Some(overlay)
}

/// Retire tous les gestionnaires connectés au signal `signal` (de
/// `GtkWidget`) de `widget`.
fn disconnect_all(widget: &gtk::Widget, signal: &str) {
    use gtk::glib::gobject_ffi;
    use gtk::glib::object::ObjectType;
    use gtk::glib::translate::IntoGlib;
    use gtk::glib::StaticType;
    let name = std::ffi::CString::new(signal).unwrap_or_default();
    unsafe {
        let id = gobject_ffi::g_signal_lookup(name.as_ptr(), gtk::Widget::static_type().into_glib());
        if id != 0 {
            gobject_ffi::g_signal_handlers_disconnect_matched(
                widget.as_ptr() as *mut gobject_ffi::GObject, gobject_ffi::G_SIGNAL_MATCH_ID, id, 0,
                std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(),
            );
        }
    }
}

fn text(props: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    props.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Charge le contenu du nœud : `url`, `file` (relatif au dossier courant) ou `html`.
fn load(view: &WebView, props: &serde_json::Map<String, Value>) {
    if let Some(html) = text(props, "html") {
        view.load_html(&html, None);
    } else if let Some(file) = text(props, "file") {
        let path = std::env::current_dir().unwrap_or_default().join(file);
        view.load_uri(&format!("file://{}", path.display()));
    } else if let Some(url) = text(props, "url") {
        view.load_uri(&url);
    }
}

/// Hôte autorisé par `allow` (liste vide ou absente : tous).
fn allowed(props: &serde_json::Map<String, Value>, uri: &str) -> bool {
    let Some(allow) = props.get("allow").and_then(Value::as_array) else { return true };
    let host = url::Url::parse(uri).ok().and_then(|u| u.host_str().map(str::to_string));
    match host {
        Some(host) => allow.iter().filter_map(Value::as_str).any(|a| host == a || host.ends_with(&format!(".{a}"))),
        None => true,
    }
}

fn create(this: i64, id: &str, overlay: &gtk::Overlay) -> WebView {
    let props = core::props(this, id);
    let manager = UserContentManager::new();
    manager.add_script(&UserScript::new(SHIM, UserContentInjectedFrames::TopFrame, UserScriptInjectionTime::Start, &[], &[]));
    manager.register_script_message_handler("ocara");
    let (owner, node) = (this, id.to_string());
    manager.connect_script_message_received(Some("ocara"), move |_, result| {
        let text = result.js_value().map(|v| v.to_str().to_string()).unwrap_or_default();
        let value = serde_json::from_str(&text).unwrap_or(Value::String(text));
        core::fire(owner, &node, "onMessage", value);
    });
    let view = WebView::with_user_content_manager(&manager);
    if let Some(settings) = WebViewExt::settings(&view) {
        settings.set_enable_javascript(props.get("javascript").and_then(Value::as_bool).unwrap_or(true));
        settings.set_enable_html5_local_storage(props.get("storage").and_then(Value::as_bool).unwrap_or(true));
        if let Some(agent) = text(&props, "userAgent") {
            settings.set_user_agent(Some(&agent));
        }
    }
    if let Some(zoom) = props.get("zoom").and_then(Value::as_f64) {
        view.set_zoom_level(zoom);
    }
    let node = id.to_string();
    view.connect_load_changed(move |view, event| {
        if event == LoadEvent::Finished {
            let url = view.uri().map(|u| u.to_string()).unwrap_or_default();
            core::post(owner, &json!({ "id": node, "prop": "url", "value": url }));
            core::post(owner, &json!({ "id": node, "prop": "canGoBack", "value": view.can_go_back() }));
            core::fire(owner, &node, "onLoad", Value::String(url));
        }
    });
    let node = id.to_string();
    view.connect_load_failed(move |_, _, uri, error| {
        core::fire(owner, &node, "onError", json!({ "url": uri, "message": error.to_string() }));
        false
    });
    let node = id.to_string();
    view.connect_decide_policy(move |_, decision, kind| {
        if !matches!(kind, PolicyDecisionType::NavigationAction | PolicyDecisionType::NewWindowAction) {
            return false;
        }
        let Some(navigation) = decision.downcast_ref::<NavigationPolicyDecision>() else { return false };
        let uri = navigation.navigation_action().and_then(|a| a.request()).and_then(|r| r.uri()).map(|u| u.to_string()).unwrap_or_default();
        let external = kind == PolicyDecisionType::NewWindowAction
            || ((uri.starts_with("http://") || uri.starts_with("https://")) && !allowed(&core::props(owner, &node), &uri));
        if !external {
            return false;
        }
        // Lien hors de `allow` (ou nouvelle fenêtre) : navigateur du système.
        decision.ignore();
        let _ = gtk::show_uri_on_window(None::<&gtk::Window>, &uri, gtk::current_event_time());
        true
    });
    view.set_halign(gtk::Align::Start);
    view.set_valign(gtk::Align::Start);
    overlay.add_overlay(&view);
    load(&view, &props);
    VIEWS.with(|views| views.borrow_mut().insert((this, id.to_string()), view.clone()));
    view
}

/// Message de rectangle du moteur : crée, place ou masque la vue `id`.
pub(crate) fn place(this: i64, message: &Value) {
    let Some(id) = message.get("webview").and_then(Value::as_str).map(str::to_string) else { return };
    let number = |key: &str| message.get(key).and_then(Value::as_f64).unwrap_or(0.0).round() as i32;
    let (x, y, width, height) = (number("x"), number("y"), number("width").max(1), number("height").max(1));
    let visible = message.get("visible").and_then(Value::as_bool).unwrap_or(false);
    on_main(this, move |window| {
        let existing = view(this, &id);
        let view = match existing {
            Some(view) => view,
            None if visible => match overlay(this, &window) {
                Some(overlay) => create(this, &id, &overlay),
                None => return,
            },
            None => return,
        };
        view.set_margin_start(x.max(0));
        view.set_margin_top(y.max(0));
        view.set_size_request(width, height);
        if visible { view.show() } else { view.hide() }
    });
}

/// `ui.set(id, prop, valeur)` sur un composant `webview` : `zoom` s'applique,
/// `url`/`file`/`html` rechargent.
pub(crate) fn apply(this: i64, id: &str, prop: &str) {
    if !matches!(prop, "url" | "file" | "html" | "zoom") {
        return;
    }
    let (id, zoom) = (id.to_string(), prop == "zoom");
    on_main(this, move |_| {
        if let Some(view) = view(this, &id) {
            let props = core::props(this, &id);
            match props.get("zoom").and_then(Value::as_f64) {
                Some(level) if zoom => view.set_zoom_level(level),
                _ => load(&view, &props),
            }
        }
    });
}

/// `ui.postMessage(id, data)` : `window.ocara.onMessage` de la page.
pub(crate) fn post_message(this: i64, id: &str, data: &Value) {
    run(this, id, format!("window.ocara && window.ocara.__receive({data})"));
}

fn run(this: i64, id: &str, script: String) {
    let id = id.to_string();
    on_main(this, move |_| {
        if let Some(view) = view(this, &id) {
            view.evaluate_javascript(&script, None, None, None::<&gtk::gio::Cancellable>, |_| {});
        }
    });
}

/// `ui.eval(id, script)` → texte du résultat (JSON pour un objet), attendu.
pub(crate) fn eval(this: i64, id: &str, script: &str) -> String {
    let (tx, rx) = mpsc::channel();
    let id = id.to_string();
    let wrapped = format!("(function () {{ var r = eval({}); return typeof r === 'string' ? r : JSON.stringify(r); }})()", json!(script));
    on_main(this, move |_| {
        let Some(view) = view(this, &id) else {
            let _ = tx.send(String::new());
            return;
        };
        view.evaluate_javascript(&wrapped, None, None, None::<&gtk::gio::Cancellable>, move |result| {
            let text = result.map(|v| if v.is_undefined() || v.is_null() { String::new() } else { v.to_str().to_string() });
            let _ = tx.send(text.unwrap_or_else(|e| format!("erreur : {e}")));
        });
    });
    crate::dialogs::wait(rx).unwrap_or_default()
}

/// `ui.webBack(id)`, `ui.webForward(id)`, `ui.reload(id)`.
pub(crate) fn navigate(this: i64, id: &str, step: &'static str) {
    let id = id.to_string();
    on_main(this, move |_| {
        if let Some(view) = view(this, &id) {
            match step {
                "back" => view.go_back(),
                "forward" => view.go_forward(),
                _ => view.reload(),
            }
        }
    });
}

/// Fenêtre fermée : ses vues disparaissent avec elle.
pub(crate) fn release(this: i64) {
    let Some(app) = crate::app::running() else { return };
    let _ = app.run_on_main_thread(move || {
        VIEWS.with(|views| views.borrow_mut().retain(|(owner, _), _| *owner != this));
        OVERLAYS.with(|o| o.borrow_mut().remove(&this));
    });
}
