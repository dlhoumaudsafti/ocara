// Raccourcis clavier globaux (plugin officiel `global-shortcut`), actifs même
// quand la fenêtre n'a pas le focus : `ui.addShortcut("CmdOrCtrl+Shift+K",
// action)` appelle les callbacks `ui.listen(action, …)` à chaque appui.

use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use ocara_runtime::ptr_to_str;

use crate::window::state;
use crate::{app, dialogs, events};

const ERR_SHORTCUT: i64 = 104;

fn register(this: i64, shortcut: &str, action: String) -> Result<(), String> {
    let Some(app) = app::running() else { return Ok(()) };
    app.global_shortcut()
        .on_shortcut(shortcut, move |_, _, event| {
            if event.state == ShortcutState::Pressed {
                events::dispatch_to_ocara(this, &serde_json::json!({ "event": action, "data": "null" }));
            }
        })
        .map_err(|e| format!("raccourci '{shortcut}' : {e}"))
}

/// Enregistre les raccourcis demandés avant `run()` (création de la fenêtre).
pub(crate) fn apply_shortcuts(this: i64) {
    for (shortcut, action) in state(this, |w| w.shortcuts.clone()).unwrap_or_default() {
        if let Err(e) = register(this, &shortcut, action) {
            eprintln!("Tauri : {e}");
        }
    }
}

/// `ui.addShortcut(shortcut, action)` : avant `run()` ou pendant.
/// `TauriException` 104 si le raccourci est invalide ou déjà pris.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_addShortcut(this: i64, shortcut_ptr: i64, action_ptr: i64) {
    let shortcut = unsafe { ptr_to_str(shortcut_ptr).to_string() };
    let action = unsafe { ptr_to_str(action_ptr).to_string() };
    state(this, |w| w.shortcuts.push((shortcut.clone(), action.clone())));
    if let Err(e) = register(this, &shortcut, action) {
        dialogs::raise(ERR_SHORTCUT, &e);
    }
}

/// `ui.removeShortcut(shortcut)`.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_removeShortcut(this: i64, shortcut_ptr: i64) {
    let shortcut = unsafe { ptr_to_str(shortcut_ptr).to_string() };
    state(this, |w| w.shortcuts.retain(|(s, _)| *s != shortcut));
    if let Some(app) = app::running() {
        let _ = app.global_shortcut().unregister(shortcut.as_str());
    }
}
