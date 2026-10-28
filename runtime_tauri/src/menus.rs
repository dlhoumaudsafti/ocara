// Menus natifs : barre de menus de la fenêtre et menu contextuel (clic
// droit), décrits par un `array<map<string, mixed>>` Ocara (`text`, `action`,
// `shortcut`, `enabled`, `checked`, `separator`, `items` pour un sous-menu).
// Un clic sur une entrée appelle les callbacks `ui.listen(action, …)` (data :
// `null`, ou `{"checked": …}` pour une entrée cochable).

use serde_json::Value;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use ocara_runtime::ptr_to_str;

use crate::window::{app, live, state};

/// Commande IPC réservée du clic droit (script `MENU_SHIM`).
pub(crate) const CONTEXT_COMMAND: &str = "__ocara_context_menu";

/// Ajouté au script d'initialisation de chaque page : remplace le menu
/// contextuel de WebKit quand `ui.setContextMenu` est actif.
pub(crate) const MENU_SHIM: &str = r#"(function() {
  window.ocara = window.ocara || {};
  document.addEventListener("contextmenu", function(event) {
    if (!window.ocara.__contextMenu) return;
    event.preventDefault();
    window.__TAURI_INTERNALS__.invoke("__ocara_context_menu", {});
  }, true);
})();
"#;

/// `items` (`array<map<string, mixed>>` Ocara) en JSON.
fn spec_of(items_ptr: i64) -> Value {
    let encoded = ocara_runtime::JSON_encode(items_ptr, 0);
    let spec = serde_json::from_str(unsafe { ptr_to_str(encoded) }).unwrap_or(Value::Null);
    ocara_runtime::rc::__rc_release(encoded);
    spec
}

/// Entrées d'une description ; l'identifiant d'une entrée est son action,
/// précédée de `prefix` (menu du tray, voir tray.rs).
fn build_items(app: &tauri::AppHandle, spec: &Value, prefix: &str, next_id: &mut usize, checks: &mut Vec<CheckMenuItem<tauri::Wry>>) -> tauri::Result<Vec<Box<dyn IsMenuItem<tauri::Wry>>>> {
    let mut items: Vec<Box<dyn IsMenuItem<tauri::Wry>>> = Vec::new();
    for entry in spec.as_array().into_iter().flatten() {
        let field = |key: &str| entry.get(key);
        let text = field("text").and_then(Value::as_str).unwrap_or_default();
        let enabled = field("enabled").and_then(Value::as_bool).unwrap_or(true);
        if field("separator").and_then(Value::as_bool).unwrap_or(false) {
            items.push(Box::new(PredefinedMenuItem::separator(app)?));
        } else if let Some(children) = field("items") {
            let submenu = Submenu::new(app, text, enabled)?;
            for child in build_items(app, children, prefix, next_id, checks)? {
                submenu.append(child.as_ref())?;
            }
            items.push(Box::new(submenu));
        } else {
            let id = match field("action").and_then(Value::as_str) {
                Some(action) => format!("{prefix}{action}"),
                None => {
                    *next_id += 1;
                    format!("{prefix}__ocara_menu_{next_id}")
                }
            };
            let shortcut = field("shortcut").and_then(Value::as_str);
            match field("checked").and_then(Value::as_bool) {
                Some(checked) => {
                    let item = CheckMenuItem::with_id(app, id, text, enabled, checked, shortcut)?;
                    checks.push(item.clone());
                    items.push(Box::new(item));
                }
                None => items.push(Box::new(MenuItem::with_id(app, id, text, enabled, shortcut)?)),
            }
        }
    }
    Ok(items)
}

/// Menu décrit par `spec`, et ses entrées cochables (lues au clic).
pub(crate) fn build_menu(app: &tauri::AppHandle, spec: &Value, prefix: &str) -> tauri::Result<(Menu<tauri::Wry>, Vec<CheckMenuItem<tauri::Wry>>)> {
    let menu = Menu::new(app)?;
    let mut checks = Vec::new();
    for item in build_items(app, spec, prefix, &mut 0, &mut checks)? {
        menu.append(item.as_ref())?;
    }
    Ok((menu, checks))
}

/// Pose la barre de menus décrite par l'état (`run()` ou `setMenu` pendant).
pub(crate) fn apply_menu_bar(this: i64, window: &tauri::WebviewWindow) {
    let Some(app) = app(this) else { return };
    let Some(spec) = state(this, |w| w.menu_spec.clone()).flatten() else { return };
    match build_menu(&app, &spec, "") {
        Ok((menu, checks)) => {
            let _ = window.set_menu(menu);
            state(this, |w| w.menu_checks = checks);
        }
        Err(e) => eprintln!("Tauri : barre de menus invalide : {e}"),
    }
}

/// Construit le menu contextuel décrit par l'état et l'active dans la page.
pub(crate) fn apply_context_menu(this: i64) {
    let Some(app) = app(this) else { return };
    let Some(spec) = state(this, |w| w.context_spec.clone()).flatten() else { return };
    match build_menu(&app, &spec, "") {
        Ok((menu, checks)) => {
            state(this, |w| {
                w.context_menu = Some(menu);
                w.context_checks = checks;
            });
        }
        Err(e) => eprintln!("Tauri : menu contextuel invalide : {e}"),
    }
    sync_page_flag(this);
}

/// Indique à la page si le clic droit est remplacé (à chaque chargement).
pub(crate) fn sync_page_flag(this: i64) {
    let active = state(this, |w| w.context_spec.is_some()).unwrap_or(false);
    if let Some(window) = live(this) {
        let _ = window.eval(format!("window.ocara && (window.ocara.__contextMenu = {active})"));
    }
}

/// Clic droit dans la page : affiche le menu contextuel au curseur.
pub(crate) fn show_context_menu(this: i64) {
    let menu = state(this, |w| w.context_menu.clone()).flatten();
    if let (Some(window), Some(menu)) = (live(this), menu) {
        let has_items = menu.items().map(|items| !items.is_empty()).unwrap_or(false);
        if has_items {
            let _ = window.popup_menu(&menu);
        }
    }
}

/// Clic sur l'entrée `id` (action `action`) : l'action devient un événement
/// `ui.listen`.
pub(crate) fn on_click(this: i64, id: &str, action: &str) {
    if action.starts_with("__ocara_menu_") || action.starts_with(crate::tray::TRAY_PREFIX) {
        return;
    }
    let checked = state(this, |w| {
        w.menu_checks.iter().chain(&w.context_checks).chain(&w.tray_checks)
            .find(|item| item.id().as_ref() == id)
            .and_then(|item| item.is_checked().ok())
    }).flatten();
    let data = match checked {
        Some(checked) => serde_json::json!({ "checked": checked }).to_string(),
        None => "null".to_string(),
    };
    crate::events::dispatch_to_ocara(this, &serde_json::json!({ "event": action, "data": data }));
}

/// `ui.setMenu(items)` : barre de menus de la fenêtre (avant ou pendant `run()`).
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setMenu(this: i64, items_ptr: i64) {
    let spec = spec_of(items_ptr);
    state(this, |w| w.menu_spec = Some(spec));
    if let Some(window) = live(this) {
        apply_menu_bar(this, &window);
    }
}

/// `ui.setContextMenu(items)` : remplace le menu du clic droit de la page ;
/// `[]` le supprime.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setContextMenu(this: i64, items_ptr: i64) {
    let spec = spec_of(items_ptr);
    state(this, |w| w.context_spec = Some(spec));
    if live(this).is_some() {
        apply_context_menu(this);
    }
}

/// `ui.setDevTools(enabled)` : autorise l'inspecteur web (avant `run()`).
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setDevTools(this: i64, enabled: i64) {
    state(this, |w| w.devtools = enabled != 0);
}

/// `ui.openDevTools()` : ouvre l'inspecteur (autorisé par `setDevTools(true)`).
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_openDevTools(this: i64) {
    if let Some(window) = live(this) {
        window.open_devtools();
    }
}
