// Icône de zone de notification (tray) : `ui.setTray(options)` — `icon`
// (fichier PNG, défaut : logo Ocara), `tooltip`, `action` (clic sur l'icône),
// `items` (menu, même description que `setMenu`). Un clic appelle les
// callbacks `ui.listen(action, …)`. Sous Linux (libayatana-appindicator),
// seul le menu est transmis, pas le clic sur l'icône elle-même.

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use crate::window::state;
use crate::{app, events, menus, options_of};

/// Préfixe des identifiants du menu du tray : `__tray:<objet>:<action>`
/// (menu de l'application, pas d'une fenêtre).
pub(crate) const TRAY_PREFIX: &str = "__tray:";

const DEFAULT_ICON: &[u8] = include_bytes!("../assets/tray-icon.png");

fn tray_id(this: i64) -> String {
    format!("ocara-tray-{this}")
}

fn dispatch(this: i64, action: &str) {
    events::dispatch_to_ocara(this, &serde_json::json!({ "event": action, "data": "null" }));
}

/// Construit l'icône décrite par l'état de `this` dans l'application en cours.
pub(crate) fn apply_tray(this: i64) {
    let Some(app) = app::running() else { return };
    let Some(spec) = state(this, |w| w.tray_spec.clone()).flatten() else { return };
    let _ = app.remove_tray_by_id(&tray_id(this));
    let text = |key: &str| spec.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let icon = match text("icon") {
        Some(path) => Image::from_path(&path).map_err(|e| format!("icône '{path}' illisible : {e}")),
        None => Image::from_bytes(DEFAULT_ICON).map_err(|e| e.to_string()),
    };
    let icon = match icon {
        Ok(icon) => icon,
        Err(e) => return eprintln!("Tauri : tray : {e}"),
    };
    let mut builder = TrayIconBuilder::with_id(tray_id(this)).icon(icon);
    if let Some(tooltip) = text("tooltip") {
        builder = builder.tooltip(tooltip);
    }
    if let Some(items) = spec.get("items") {
        match menus::build_menu(&app, items, &format!("{TRAY_PREFIX}{this}:")) {
            Ok((menu, checks)) => {
                builder = builder.menu(&menu);
                state(this, |w| w.tray_checks = checks);
            }
            Err(e) => eprintln!("Tauri : menu du tray invalide : {e}"),
        }
    }
    if let Some(action) = text("action") {
        builder = builder.on_tray_icon_event(move |_, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                dispatch(this, &action);
            }
        });
    }
    match builder.build(&app) {
        Ok(tray) => {
            state(this, |w| w.tray = Some(tray));
        }
        Err(e) => eprintln!("Tauri : tray impossible à créer : {e}"),
    }
}

/// Clic dans un menu de l'application : seul celui du tray nous concerne
/// (les menus de fenêtre passent par `on_menu_event` de leur fenêtre).
pub(crate) fn on_app_menu_event(id: &str) {
    let Some(rest) = id.strip_prefix(TRAY_PREFIX) else { return };
    let Some((owner, action)) = rest.split_once(':') else { return };
    if let Ok(this) = owner.parse::<i64>() {
        menus::on_click(this, id, action);
    }
}

/// Retire l'icône de `this` (fenêtre libérée, `removeTray`).
pub(crate) fn remove(this: i64) {
    if let Some(app) = app::running() {
        let _ = app.remove_tray_by_id(&tray_id(this));
    }
}

/// `ui.setTray(options)` : avant `run()` ou pendant.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setTray(this: i64, options_ptr: i64) {
    let spec = options_of(options_ptr);
    state(this, |w| w.tray_spec = Some(spec));
    apply_tray(this);
}

/// `ui.removeTray()` : retire l'icône.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_removeTray(this: i64) {
    remove(this);
    state(this, |w| {
        w.tray_spec = None;
        w.tray = None;
        w.tray_checks.clear();
    });
}
