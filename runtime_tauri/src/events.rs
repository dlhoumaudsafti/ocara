// Événements entre la page et Ocara. `ui.emit(event, data)` encode `data` en
// JSON et le remet aux écouteurs `window.ocara.on(event, cb)` de la page ;
// `window.ocara.emit(event, data)` (page) appelle chaque callback
// `ui.listen(event, cb)` avec le texte JSON de `data`.

use ocara_runtime::{alloc_str, ptr_to_str};

use crate::window::{live, state};

unsafe extern "C" {
    /// runtime/src/lib.rs : appel rattrapé par la pile des `try` du runtime
    /// du programme, pas par celle de la copie liée dans ce crate.
    fn __call_closure_catching(fat_ptr: i64, arg: i64) -> i64;
}

/// Commande IPC réservée de `window.ocara.emit`.
pub(crate) const EVENT_COMMAND: &str = "__ocara_event";

/// Ajouté au script d'initialisation de chaque page (`build_ipc_shim_script`).
pub(crate) const EVENTS_SHIM: &str = r#"(function() {
  window.ocara = window.ocara || {};
  var listeners = {};
  window.ocara.on = function(event, callback) {
    (listeners[event] = listeners[event] || []).push(callback);
    return function() {
      listeners[event] = (listeners[event] || []).filter(function(c) { return c !== callback; });
    };
  };
  window.ocara.emit = function(event, data) {
    return window.__TAURI_INTERNALS__.invoke("__ocara_event", {
      event: event,
      data: JSON.stringify(data === undefined ? null : data)
    });
  };
  window.ocara.__dispatch = function(event, data) {
    (listeners[event] || []).slice().forEach(function(callback) {
      try { callback(data); } catch (e) { console.error(e); }
    });
  };
})();
"#;

/// `ui.listen(event, callback)` : `callback` (`Function<void(string)>`) reçoit
/// le texte JSON des données de chaque `window.ocara.emit(event, data)`.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_listen(this: i64, event_ptr: i64, callback_ptr: i64) {
    let event = unsafe { ptr_to_str(event_ptr).to_string() };
    // Référence gardée par l'objet (`__kept`), pas par l'état natif : un
    // callback qui capture sa fenêtre forme un cycle visible du collecteur.
    if state(this, |w| w.event_callbacks.entry(event).or_default().push(callback_ptr)).is_some() {
        ocara_runtime::rc::__rc_keep(this, 1, callback_ptr);
    }
}

/// `ui.emit(event, data)` : remet `data` (encodé en JSON) aux écouteurs de la
/// page. Sans effet avant `run()` (aucune page chargée).
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_emit(this: i64, event_ptr: i64, data_ptr: i64) {
    let Some(window) = live(this) else { return };
    let event = unsafe { ptr_to_str(event_ptr) };
    let encoded = ocara_runtime::JSON_encode(data_ptr, 0);
    let data = unsafe { ptr_to_str(encoded) }.to_string();
    ocara_runtime::rc::__rc_release(encoded);
    let event = serde_json::to_string(event).unwrap_or_else(|_| "\"\"".to_string());
    let _ = window.eval(format!("window.ocara && window.ocara.__dispatch({event}, {data})"));
}

/// Appelle les callbacks `ui.listen` de l'événement porté par `payload`
/// (`{event, data}`, data : texte JSON). Une exception d'un callback est
/// signalée sur stderr : la boucle de Tauri continue.
pub(crate) fn dispatch_to_ocara(this: i64, payload: &serde_json::Value) {
    let event = payload.get("event").and_then(|v| v.as_str()).unwrap_or_default();
    let data = match payload.get("data") {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
        None => "null".to_string(),
    };
    let callbacks = state(this, |w| w.event_callbacks.get(event).cloned().unwrap_or_default()).unwrap_or_default();
    for callback in callbacks {
        let _running = ocara_runtime::rc::enter();
        let arg = unsafe { alloc_str(&data) };
        let uncaught = unsafe { __call_closure_catching(callback, arg) };
        ocara_runtime::rc::__rc_release(arg);
        if uncaught != 0 {
            eprintln!("Tauri : écouteur de '{event}' : exception non rattrapée ({})", unsafe { ptr_to_str(uncaught) });
            ocara_runtime::rc::__rc_release(uncaught);
        }
    }
}

