// Boucle d'événements et fenêtres réelles. Le premier `run()` construit
// l'application Tauri et sa fenêtre ; pendant la boucle, `open()` (ou
// `run()`) d'un autre objet `Tauri` construit la sienne. Chaque fenêtre garde
// ses handlers, écouteurs et menus : appels IPC et clics de menu sont
// aiguillés vers l'objet Ocara d'après l'étiquette de la vraie fenêtre.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use tauri::Manager;

use ocara_runtime::{alloc_str, free_str, ptr_to_str};

use crate::window::{live, state, MAIN_LABEL};
use crate::{build_ipc_shim_script, dialogs, events, lookup_handler, menus, TAURI_WINDOWS};

/// Application en cours (`run()`), pour ouvrir d'autres fenêtres.
static APP: Mutex<Option<tauri::AppHandle>> = Mutex::new(None);
static NEXT_WINDOW: AtomicUsize = AtomicUsize::new(1);

pub(crate) fn running() -> Option<tauri::AppHandle> {
    APP.lock().unwrap().clone()
}

/// Objet `Tauri` de la vraie fenêtre `label`.
fn owner_of(label: &str) -> Option<i64> {
    let windows = TAURI_WINDOWS.lock().unwrap();
    windows.iter().find(|(_, win)| win.lock().unwrap().label == label).map(|(&this, _)| this)
}

/// Implémentation de `tauri::Assets<R>` qui lit les fichiers du frontend
/// directement sur le disque (au lieu de les embarquer à la compilation du
/// runtime, ce qui serait figé pour tous les programmes Ocara).
/// Base = répertoire courant du processus au moment de `run()`.
struct DiskAssets {
    base_dir: std::path::PathBuf,
}

impl<R: tauri::Runtime> tauri::Assets<R> for DiskAssets {
    fn get(&self, key: &tauri::utils::assets::AssetKey) -> Option<std::borrow::Cow<'_, [u8]>> {
        let rel = key.as_ref().trim_start_matches('/');
        if rel == crate::ui::PAGE {
            return Some(std::borrow::Cow::Owned(crate::ui::page().into_bytes()));
        }
        let rel = if rel.is_empty() { "index.html" } else { rel };
        std::fs::read(self.base_dir.join(rel)).ok().map(std::borrow::Cow::Owned)
    }

    fn iter(&self) -> Box<tauri::utils::assets::AssetsIter<'_>> {
        Box::new(std::iter::empty())
    }

    fn csp_hashes(&self, _html_path: &tauri::utils::assets::AssetKey) -> Box<dyn Iterator<Item = tauri::utils::assets::CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

/// Retire les variables GTK que des applications tierces (VS Code en snap)
/// injectent dans l'environnement : GTK chargerait leurs modules, incompatibles
/// avec la glibc du système (`undefined symbol: __libc_pthread_init`).
fn clear_inherited_gtk_env() {
    for var in ["GTK_PATH", "GTK_EXE_PREFIX", "GDK_PIXBUF_MODULE_FILE", "GDK_PIXBUF_MODULEDIR", "GIO_MODULE_DIR", "GTK_IM_MODULE_FILE"] {
        unsafe { std::env::remove_var(var); }
    }
}

/// `url` : fichier local servi par `DiskAssets`, ou URL http(s) externe
/// (serveur `HTTPServer` Ocara).
fn webview_url(url: &str) -> tauri::WebviewUrl {
    match url::Url::parse(url) {
        Ok(u) if u.scheme() == "http" || u.scheme() == "https" => tauri::WebviewUrl::External(u),
        _ => tauri::WebviewUrl::App(url.into()),
    }
}

/// Construit la vraie fenêtre de l'objet `this` d'après son état (options
/// de `use Tauri`, réglages faits avant), son script d'initialisation, ses
/// menus.
pub(crate) fn build_window<M: Manager<tauri::Wry>>(manager: &M, this: i64, label: String) -> tauri::Result<tauri::WebviewWindow> {
    let config = state(this, |w| {
        w.label = label.clone();
        w.clone_config()
    }).ok_or(tauri::Error::WindowNotFound)?;
    let mut builder = tauri::WebviewWindowBuilder::new(manager, &label, webview_url(&config.url))
        .title(&config.title)
        .inner_size(config.width as f64, config.height as f64)
        .maximized(config.maximized)
        .decorations(config.decorations)
        .always_on_top(config.always_on_top)
        .transparent(config.transparent)
        .resizable(config.resizable)
        .initialization_script(&build_ipc_shim_script(this))
        .devtools(config.devtools)
        .on_page_load(move |_, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                menus::sync_page_flag(this);
            }
        });
    if let Some((width, height)) = config.min_size {
        builder = builder.min_inner_size(width as f64, height as f64);
    }
    if config.center {
        builder = builder.center();
    }
    if let Some(tree) = &config.ui_tree {
        builder = builder.initialization_script(&crate::ui::boot_script(tree));
    }
    let window = builder.build()?;
    if config.minimized {
        let _ = window.minimize();
    }
    crate::window::track_size(this, &window);
    let handle = manager.app_handle().clone();
    state(this, |w| w.app = Some(handle));
    window.on_menu_event(move |_, event| menus::on_click(this, event.id().as_ref(), event.id().as_ref()));
    menus::apply_menu_bar(this, &window);
    menus::apply_context_menu(this);
    crate::tray::apply_tray(this);
    crate::shortcuts::apply_shortcuts(this);
    Ok(window)
}

/// Fenêtre secondaire : construite dans l'application en cours.
pub(crate) fn open_secondary(this: i64, app: &tauri::AppHandle) {
    let label = format!("window-{}", NEXT_WINDOW.fetch_add(1, Ordering::SeqCst));
    if let Err(e) = build_window(app, this, label) {
        eprintln!("Tauri : fenêtre impossible à ouvrir : {e}");
    }
}

/// ACL : commandes des handlers de toutes les fenêtres (enregistrés avant le
/// premier `run()`) et commandes réservées, depuis toute origine http(s) —
/// Tauri v2 refuse sinon toute commande d'une page « distante », y compris
/// http://localhost.
fn authority() -> tauri::ipc::RuntimeAuthority {
    use tauri::utils::acl::resolved::{Resolved, ResolvedCommand};
    let mut names: Vec<String> = TAURI_WINDOWS.lock().unwrap().values()
        .flat_map(|win| win.lock().unwrap().handlers.keys().cloned().collect::<Vec<_>>())
        .collect();
    names.extend([events::EVENT_COMMAND.to_string(), menus::CONTEXT_COMMAND.to_string(), crate::ui::EVENT_COMMAND.to_string()]);
    let any_origin = tauri::utils::acl::ExecutionContext::Remote {
        url: "http://*:*/*".parse().expect("motif ACL http://*:*/* invalide"),
    };
    let match_all = glob::Pattern::new("*").expect("motif glob * invalide");
    let allowed_commands = names.into_iter()
        .map(|name| (name, vec![ResolvedCommand {
            context: any_origin.clone(),
            windows: vec![match_all.clone()],
            webviews: vec![match_all.clone()],
            scope_id: None,
            ..Default::default()
        }]))
        .collect();
    let resolved = Resolved { has_app_acl: false, allowed_commands, ..Default::default() };
    #[cfg(debug_assertions)]
    return tauri::ipc::RuntimeAuthority::new(std::collections::BTreeMap::new(), resolved);
    #[cfg(not(debug_assertions))]
    return tauri::ipc::RuntimeAuthority::new(resolved);
}

/// Appel IPC de la page : commande réservée, ou handler Ocara de la fenêtre
/// émettrice (trampoline généré par le compilateur, réponse JSON).
fn invoke(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    let command = invoke.message.command().to_string();
    let Some(this) = owner_of(invoke.message.webview().label()) else {
        invoke.resolver.reject("fenêtre Ocara introuvable".to_string());
        return true;
    };
    let args_json = match invoke.message.payload() {
        tauri::ipc::InvokeBody::Json(v) => v.to_string(),
        // Un payload binaire ne vient que d'un tableau passé directement à
        // __TAURI_INTERNALS__.invoke (voir build_ipc_shim_script) : {} laisse
        // chaque argument manquant, rejeté proprement par le trampoline.
        tauri::ipc::InvokeBody::Raw(_) => "{}".to_string(),
    };
    if command == menus::CONTEXT_COMMAND {
        menus::show_context_menu(this);
        invoke.resolver.resolve(serde_json::Value::Null);
        return true;
    }
    if command == crate::ui::EVENT_COMMAND {
        crate::ui::receive(this, &args_json);
        invoke.resolver.resolve(serde_json::Value::Null);
        return true;
    }
    if command == events::EVENT_COMMAND {
        let payload = serde_json::from_str(&args_json).unwrap_or(serde_json::Value::Null);
        events::dispatch_to_ocara(this, &payload);
        invoke.resolver.resolve(serde_json::Value::Null);
        return true;
    }
    let Some(trampoline_addr) = lookup_handler(this, &command) else {
        invoke.resolver.reject(format!("aucun handler enregistré pour la commande '{command}'"));
        return true;
    };
    // La boucle est garée (voir `Tauri_run`) : elle compte comme en cours le
    // temps du handler Ocara.
    let _running = ocara_runtime::rc::enter();
    let args_ptr = unsafe { alloc_str(&args_json) };
    let result_ptr = unsafe {
        let f: unsafe extern "C" fn(i64) -> i64 = std::mem::transmute(trampoline_addr as usize);
        f(args_ptr)
    };
    unsafe { free_str(args_ptr) };
    let result = unsafe { ptr_to_str(result_ptr) }.to_string();
    unsafe { free_str(result_ptr) };
    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(serde_json::Value::Object(obj)) if obj.get("ok").and_then(|v| v.as_bool()) == Some(true) => {
            invoke.resolver.resolve(obj.get("value").cloned().unwrap_or(serde_json::Value::Null));
        }
        Ok(serde_json::Value::Object(obj)) => {
            let message = obj.get("error").and_then(|v| v.as_str()).unwrap_or("erreur inconnue").to_string();
            invoke.resolver.reject(message);
        }
        _ => invoke.resolver.reject("réponse invalide du handler Ocara".to_string()),
    }
    true
}

/// `ui.run()`. Premier appel : lance l'application et sa fenêtre, bloque
/// jusqu'à la fermeture de toutes les fenêtres (thread principal requis).
/// Pendant la boucle : ouvre la fenêtre de cet objet et, hors du thread de
/// la boucle, attend sa fermeture.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_run(this: i64) {
    if ocara_runtime::blocking::neutralized("Tauri.run") {
        return;
    }
    if let Some(app) = running() {
        if live(this).is_none() {
            open_secondary(this, &app);
        }
        if !dialogs::on_loop_thread() {
            while live(this).is_some() {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        return;
    }
    clear_inherited_gtk_env();
    dialogs::note_loop_thread();

    let mut config = tauri::utils::config::Config::default();
    config.identifier = "app.ocara.demo".to_string();
    let assets: Box<dyn tauri::Assets<tauri::Wry>> = Box::new(DiskAssets {
        base_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    });
    let package_info = tauri::PackageInfo {
        name: "Ocara App".to_string(),
        version: semver::Version::new(0, 1, 0),
        authors: "",
        description: "",
        crate_name: "ocara_app",
    };
    let context = tauri::Context::new(config, assets, None, None, package_info, tauri::Pattern::Brownfield, authority(), None);

    let setup = move |app: &mut tauri::App<tauri::Wry>| -> Result<(), Box<dyn std::error::Error>> {
        *APP.lock().unwrap() = Some(app.handle().clone());
        app.on_menu_event(|_, event| crate::tray::on_app_menu_event(event.id().as_ref()));
        build_window(app, this, MAIN_LABEL.to_string())?;
        Ok(())
    };

    // Garé pendant toute la boucle : les autres threads (serveur HTTP)
    // collectent leurs cycles ; un handler Ocara se recompte. `run_return` et
    // non `run` : `run` termine le processus à la fermeture de la dernière
    // fenêtre, et le code qui suit `ui.run()` ne s'exécutait jamais.
    let parked = ocara_runtime::rc::park();
    match tauri::Builder::<tauri::Wry>::new()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(invoke)
        .setup(setup)
        .build(context) {
        Ok(app) => {
            app.run_return(|_, _| {});
        }
        Err(e) => eprintln!("[Tauri_run] erreur au lancement : {e}"),
    }
    drop(parked);

    *APP.lock().unwrap() = None;
    for win in TAURI_WINDOWS.lock().unwrap().values() {
        win.lock().unwrap().app = None;
    }
    crate::finalize_window(this);
}
