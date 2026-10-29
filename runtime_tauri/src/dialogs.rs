// Boîtes de dialogue et notifications natives (plugins officiels `dialog` et
// `notification` de Tauri), sous les noms standard d'`ocara.UI` : alert,
// confirm, pickFile, saveFile, notify.

use std::sync::OnceLock;
use std::sync::mpsc;
use std::thread::ThreadId;

use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::NotificationExt;

use ocara_runtime::{alloc_str, ptr_to_str};

use crate::window::{app, live};

const ERR_NOT_RUNNING: i64 = 102;
const ERR_NOTIFICATION: i64 = 103;

unsafe extern "C" {
    /// runtime/src/exception.rs, en ABI C : la pile des `try` du programme,
    /// pas celle de la copie du runtime liée dans ce crate.
    fn __raise_builtin_exception(
        class: *const u8, class_len: usize,
        source: *const u8, source_len: usize,
        message: *const u8, message_len: usize,
        code: i64,
    ) -> !;
}

/// Lève `TauriException` (`code`, `message`).
pub(crate) fn raise(code: i64, message: &str) -> ! {
    let (class, source) = ("TauriException", "Tauri");
    unsafe {
        __raise_builtin_exception(
            class.as_ptr(), class.len(),
            source.as_ptr(), source.len(),
            message.as_ptr(), message.len(),
            code,
        )
    }
}

/// Thread de la boucle d'événements de Tauri (noté par `Tauri_run`).
static LOOP_THREAD: OnceLock<ThreadId> = OnceLock::new();

pub(crate) fn note_loop_thread() {
    let _ = LOOP_THREAD.set(std::thread::current().id());
}

/// Vrai sur le thread de la boucle de Tauri (handler, écouteur).
pub(crate) fn on_loop_thread() -> bool {
    LOOP_THREAD.get() == Some(&std::thread::current().id())
}

fn running_app(this: i64, what: &str) -> tauri::AppHandle {
    app(this).unwrap_or_else(|| raise(ERR_NOT_RUNNING, &format!("{what} : la fenêtre n'est pas ouverte (appeler pendant run())")))
}

/// Attend la réponse d'une boîte. Sur le thread de la boucle (handler,
/// callback `listen`), un blocage simple empêcherait la boîte de s'afficher :
/// la boucle GTK tourne le temps de l'attente.
pub(crate) fn wait<T>(answer: mpsc::Receiver<T>) -> Option<T> {
    #[cfg(target_os = "linux")]
    if on_loop_thread() {
        loop {
            match answer.try_recv() {
                Ok(value) => return Some(value),
                Err(mpsc::TryRecvError::Empty) => {
                    gtk::main_iteration_do(true);
                }
                Err(mpsc::TryRecvError::Disconnected) => return None,
            }
        }
    }
    answer.recv().ok()
}

fn message(this: i64, what: &str, title: &str, text: &str, kind: MessageDialogKind, buttons: MessageDialogButtons) -> bool {
    let app = running_app(this, what);
    let mut builder = app.dialog().message(text).title(title).kind(kind).buttons(buttons);
    if let Some(window) = live(this) {
        builder = builder.parent(&window);
    }
    let (tx, rx) = mpsc::channel();
    builder.show(move |ok| {
        let _ = tx.send(ok);
    });
    wait(rx).unwrap_or(false)
}

fn text_arg(ptr: i64) -> String {
    if ptr == 0 { String::new() } else { unsafe { ptr_to_str(ptr).to_string() } }
}

/// `ui.alert(title, text)` : message d'information, bloquant jusqu'à sa fermeture.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_alert(this: i64, title_ptr: i64, text_ptr: i64) {
    message(this, "alert", &text_arg(title_ptr), &text_arg(text_ptr), MessageDialogKind::Info, MessageDialogButtons::Ok);
}

/// `ui.confirm(title, text)` → `bool` : vrai si l'utilisateur valide.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_confirm(this: i64, title_ptr: i64, text_ptr: i64) -> i64 {
    message(this, "confirm", &text_arg(title_ptr), &text_arg(text_ptr), MessageDialogKind::Info, MessageDialogButtons::OkCancel) as i64
}

/// Extensions d'un `array<string>` Ocara (`["png", "jpg"]`).
fn extensions(array: i64) -> Vec<String> {
    if array == 0 {
        return Vec::new();
    }
    (0..ocara_runtime::__array_len(array))
        .map(|i| ocara_runtime::__array_get(array, i))
        .filter(|&item| item != 0)
        .map(|item| unsafe { ptr_to_str(item) }.trim_start_matches('.').to_string())
        .collect()
}

fn path_result(path: Option<tauri_plugin_dialog::FilePath>) -> i64 {
    let text = path.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()).unwrap_or_default();
    unsafe { alloc_str(&text) }
}

/// `ui.pickFile(extensions)` → `string` : chemin choisi, `""` si annulé.
/// Un tableau vide accepte tous les fichiers.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_pickFile(this: i64, extensions_ptr: i64) -> i64 {
    let app = running_app(this, "pickFile");
    let mut builder = app.dialog().file();
    let wanted = extensions(extensions_ptr);
    if !wanted.is_empty() {
        let refs: Vec<&str> = wanted.iter().map(String::as_str).collect();
        builder = builder.add_filter(wanted.join(", "), &refs);
    }
    if let Some(window) = live(this) {
        builder = builder.set_parent(&window);
    }
    let (tx, rx) = mpsc::channel();
    builder.pick_file(move |path| {
        let _ = tx.send(path);
    });
    path_result(wait(rx).flatten())
}

/// `ui.saveFile(name)` → `string` : chemin d'enregistrement choisi (nom
/// proposé : `name`), `""` si annulé.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_saveFile(this: i64, name_ptr: i64) -> i64 {
    let app = running_app(this, "saveFile");
    let mut builder = app.dialog().file().set_file_name(text_arg(name_ptr));
    if let Some(window) = live(this) {
        builder = builder.set_parent(&window);
    }
    let (tx, rx) = mpsc::channel();
    builder.save_file(move |path| {
        let _ = tx.send(path);
    });
    path_result(wait(rx).flatten())
}

/// `ui.notify(title, text)` : notification système.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_notify(this: i64, title_ptr: i64, text_ptr: i64) {
    let app = running_app(this, "notify");
    let shown = app.notification().builder().title(text_arg(title_ptr)).body(text_arg(text_ptr)).show();
    if let Err(e) = shown {
        raise(ERR_NOTIFICATION, &format!("notification impossible : {e}"));
    }
}

/// `ui.dialog(options)` → `string` (forme d'origine, conservée) : `type`
/// `"info"`/`"warning"`/`"error"` (message, rend `"ok"`), `"confirm"` (rend
/// `"ok"` ou `"cancel"`), `"open"` / `"save"` (rend le chemin, `""` si annulé) ;
/// `title`, `message`.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_dialog(this: i64, options_ptr: i64) -> i64 {
    let option = |key: &str| {
        let value = ocara_runtime::map_lookup(options_ptr, key);
        text_arg(value)
    };
    let (title, text) = (option("title"), option("message"));
    let answer = match option("type").as_str() {
        "confirm" => {
            let ok = message(this, "dialog", &title, &text, MessageDialogKind::Info, MessageDialogButtons::OkCancel);
            if ok { "ok" } else { "cancel" }.to_string()
        }
        "open" => return Tauri_pickFile(this, 0),
        "save" => return Tauri_saveFile(this, 0),
        kind => {
            let kind = match kind {
                "warning" => MessageDialogKind::Warning,
                "error" => MessageDialogKind::Error,
                _ => MessageDialogKind::Info,
            };
            message(this, "dialog", &title, &text, kind, MessageDialogButtons::Ok);
            "ok".to_string()
        }
    };
    unsafe { alloc_str(&answer) }
}
