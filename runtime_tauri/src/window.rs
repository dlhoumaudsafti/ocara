// État de la fenêtre. Avant `run()`, ces méthodes règlent la fenêtre à
// ouvrir ; pendant `run()`, elles lisent et modifient la vraie fenêtre, depuis
// n'importe quel thread : Tauri traite l'appel sur sa boucle d'événements
// (directement si l'appelant est déjà ce thread, cas d'un handler IPC).

use tauri::{LogicalSize, Manager};

use ocara_runtime::{alloc_str, ptr_to_str};

use crate::{OcaraTauriWindow, TAURI_WINDOWS};

/// Étiquette de la fenêtre créée par `Tauri_run`.
pub(crate) const MAIN_LABEL: &str = "main";

/// Applique `f` à l'état de la fenêtre `this`, verrou de la table relâché.
pub(crate) fn state<R>(this: i64, f: impl FnOnce(&mut OcaraTauriWindow) -> R) -> Option<R> {
    let win = TAURI_WINDOWS.lock().unwrap().get(&this)?.clone();
    let mut w = win.lock().unwrap();
    Some(f(&mut w))
}

/// Vraie fenêtre, si `run()` l'a ouverte et qu'elle existe encore.
fn live(this: i64) -> Option<tauri::WebviewWindow> {
    let app = state(this, |w| w.app.clone())??;
    app.get_webview_window(MAIN_LABEL)
}

/// Taille intérieure réelle en pixels logiques (ceux de `use Tauri`).
fn logical_size(window: &tauri::WebviewWindow) -> Option<(i64, i64)> {
    let scale = window.scale_factor().ok()?;
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    Some((size.width.round() as i64, size.height.round() as i64))
}

fn flag(value: bool) -> i64 {
    value as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getTitle(this: i64) -> i64 {
    let title = live(this)
        .and_then(|w| w.title().ok())
        .or_else(|| state(this, |w| w.title.clone()));
    title.map_or(0, |t| unsafe { alloc_str(&t) })
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setTitle(this: i64, title_ptr: i64) {
    let title = unsafe { ptr_to_str(title_ptr).to_string() };
    if let Some(window) = live(this) {
        let _ = window.set_title(&title);
    }
    state(this, |w| w.title = title);
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getWidth(this: i64) -> i64 {
    live(this).and_then(|w| logical_size(&w)).map(|(width, _)| width)
        .or_else(|| state(this, |w| w.width))
        .unwrap_or(800)
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setWidth(this: i64, width: i64) {
    resize(this, Some(width), None);
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getHeight(this: i64) -> i64 {
    live(this).and_then(|w| logical_size(&w)).map(|(_, height)| height)
        .or_else(|| state(this, |w| w.height))
        .unwrap_or(600)
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setHeight(this: i64, height: i64) {
    resize(this, None, Some(height));
}

/// Change une dimension, l'autre restant celle tenue par l'état (gardé à jour
/// par `track_size`) : la taille réelle ne change qu'au traitement de l'appel
/// précédent par la boucle de Tauri.
fn resize(this: i64, width: Option<i64>, height: Option<i64>) {
    let size = state(this, |w| {
        if let Some(width) = width { w.width = width; }
        if let Some(height) = height { w.height = height; }
        (w.width, w.height)
    });
    if let (Some(window), Some((width, height))) = (live(this), size) {
        let _ = window.set_size(LogicalSize::new(width as f64, height as f64));
    }
}

/// Tient la taille de l'état à jour des redimensionnements de la vraie
/// fenêtre (à la souris compris).
pub(crate) fn track_size(this: i64, window: &tauri::WebviewWindow) {
    let observed = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Resized(_) = event {
            if let Some((width, height)) = logical_size(&observed) {
                state(this, |w| {
                    w.width = width;
                    w.height = height;
                });
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getUrl(this: i64) -> i64 {
    let url = live(this)
        .and_then(|w| w.url().ok().map(|u| u.to_string()))
        .or_else(|| state(this, |w| w.url.clone()));
    url.map_or(0, |u| unsafe { alloc_str(&u) })
}

/// Pendant `run()`, une URL relative (`"autre.html"`) se résout par rapport à
/// la page courante, comme un lien.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setUrl(this: i64, url_ptr: i64) {
    let url = unsafe { ptr_to_str(url_ptr).to_string() };
    if let Some(window) = live(this) {
        let target = url::Url::parse(&url).ok()
            .or_else(|| window.url().ok().and_then(|current| current.join(&url).ok()));
        if let Some(target) = target {
            let _ = window.navigate(target);
        }
    }
    state(this, |w| w.url = url);
}

/// Affiche la fenêtre au premier plan, réduite ou non. Sous Linux, GNOME
/// ignore la désiconification (`unminimize`) et `set_focus` ne fait rien sur
/// une fenêtre réduite : `present_with_time` de GTK, sur le thread principal,
/// fait les deux (`linux_present`).
fn bring_to_front(window: &tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();
    #[cfg(target_os = "linux")]
    {
        let target = window.clone();
        let _ = window.run_on_main_thread(move || {
            if let Ok(gtk_window) = target.gtk_window() {
                linux_present(&gtk_window);
            }
        });
    }
    let _ = window.set_focus();
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn gdk_x11_get_server_time(window: *mut gtk::gdk::ffi::GdkWindow) -> u32;
}

/// `present()` sans heure d'interaction : la prévention du vol de focus de
/// GNOME se contente alors de marquer la fenêtre « demande l'attention ».
/// Sous X11, l'heure courante du serveur X la fait réellement afficher.
#[cfg(target_os = "linux")]
fn linux_present(gtk_window: &gtk::ApplicationWindow) {
    use gtk::glib::object::ObjectExt;
    use gtk::glib::translate::ToGlibPtr;
    use gtk::prelude::{GtkWindowExt, WidgetExt};
    let time = gtk_window.window()
        .filter(|gdk_window| gdk_window.type_().name() == "GdkX11Window")
        .map(|gdk_window| unsafe { gdk_x11_get_server_time(gdk_window.to_glib_none().0) });
    match time {
        Some(time) => gtk_window.present_with_time(time),
        None => gtk_window.present(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_open(this: i64) {
    if let Some(window) = live(this) {
        bring_to_front(&window);
    }
    state(this, |w| {
        w.is_open = true;
        w.is_minimized = false;
    });
}

/// Pendant `run()`, ferme la vraie fenêtre : `run()` rend alors la main.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_close(this: i64) {
    if let Some(window) = live(this) {
        let _ = window.close();
    }
    state(this, |w| w.is_open = false);
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_isOpen(this: i64) -> i64 {
    let running = state(this, |w| w.app.is_some()).unwrap_or(false);
    if running {
        return flag(live(this).is_some_and(|w| w.is_visible().unwrap_or(false)));
    }
    flag(state(this, |w| w.is_open).unwrap_or(false))
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_focus(this: i64) {
    if let Some(window) = live(this) {
        let _ = window.set_focus();
    }
    state(this, |w| w.has_focus = true);
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_minimize(this: i64) {
    if let Some(window) = live(this) {
        let _ = window.minimize();
    }
    state(this, |w| {
        w.is_minimized = true;
        w.is_maximized = false;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_maximize(this: i64) {
    if let Some(window) = live(this) {
        let _ = window.maximize();
    }
    state(this, |w| {
        w.is_maximized = true;
        w.is_minimized = false;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_restore(this: i64) {
    if let Some(window) = live(this) {
        let _ = window.unmaximize();
        if window.is_minimized().unwrap_or(false) {
            bring_to_front(&window);
        }
    }
    state(this, |w| {
        w.is_minimized = false;
        w.is_maximized = false;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_hasFocus(this: i64) -> i64 {
    match live(this) {
        Some(window) => flag(window.is_focused().unwrap_or(false)),
        None => flag(state(this, |w| w.has_focus).unwrap_or(false)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_isMinimized(this: i64) -> i64 {
    match live(this) {
        Some(window) => flag(window.is_minimized().unwrap_or(false)),
        None => flag(state(this, |w| w.is_minimized).unwrap_or(false)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Tauri_isMaximized(this: i64) -> i64 {
    match live(this) {
        Some(window) => flag(window.is_maximized().unwrap_or(false)),
        None => flag(state(this, |w| w.is_maximized).unwrap_or(false)),
    }
}
