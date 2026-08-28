
// runtime/src/tauri.rs — squelette structuré pour intégration Tauri native
// Ce fichier prépare tous les points d'extension pour brancher la logique Tauri (Rust natif)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

use crate::{alloc_str, ptr_to_str, __map_get};

// Structure représentant une fenêtre Tauri côté Ocara
struct OcaraTauriWindow {
    title: String,
    width: i64,
    height: i64,
    url: String,
    is_open: bool,
    is_minimized: bool,
    is_maximized: bool,
    has_focus: bool,
    // Gestion des callbacks JS → Ocara
    event_callbacks: HashMap<String, i64>, // event_name → callback_ptr
    // TODO: Ajouter les handles natifs Tauri si besoin
}

// Table globale des fenêtres créées (clé = pointeur Ocara)
// Convention du projet : once_cell::sync::Lazy plutôt que lazy_static (cf. dotenv.rs)
static TAURI_WINDOWS: Lazy<Mutex<HashMap<i64, Arc<Mutex<OcaraTauriWindow>>>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

/// Lit une clé string d'une map d'options (`__map_get` + décodage du pointeur
/// string), ou renvoie `default` si la clé est absente (valeur brute nulle).
unsafe fn map_get_str(options_ptr: i64, key: &str, default: &str) -> String {
    let key_ptr = unsafe { alloc_str(key) };
    let val = __map_get(options_ptr, key_ptr);
    if val == 0 {
        default.to_string()
    } else {
        unsafe { ptr_to_str(val).to_string() }
    }
}

/// Lit une clé int d'une map d'options, ou renvoie `default` si absente (0).
unsafe fn map_get_int(options_ptr: i64, key: &str, default: i64) -> i64 {
    let key_ptr = unsafe { alloc_str(key) };
    let val = __map_get(options_ptr, key_ptr);
    if val == 0 { default } else { val }
}

/// Crée une nouvelle fenêtre Tauri avec les options (title, width, height, url...).
///
/// `this` est le pointeur alloué par `use Tauri(...)` (voir Expr::New dans
/// src/lower/expr.d/lower.rs, qui appelle systématiquement `<Classe>_init(this, ...)` —
/// c'est pourquoi ce constructeur doit s'appeler `Tauri_init`, pas `Tauri___init__` :
/// tout autre nom est silencieusement ignoré par le codegen, qui déclare l'appel
/// vers `Tauri_init` sans jamais vérifier qu'un symbole portant ce nom existe).
/// On stocke l'état de la fenêtre dans TAURI_WINDOWS en utilisant CE pointeur comme
/// clé — c'est lui, et lui seul, qui est repassé en `this` à tous les appels
/// `ui.method()` suivants.
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_init(this: i64, options_ptr: i64) {
    let win = Arc::new(Mutex::new(OcaraTauriWindow {
        title:  unsafe { map_get_str(options_ptr, "title", "Ocara App") },
        width:  unsafe { map_get_int(options_ptr, "width", 800) },
        height: unsafe { map_get_int(options_ptr, "height", 600) },
        url:    unsafe { map_get_str(options_ptr, "url", "index.html") },
        is_open: true,
        is_minimized: false,
        is_maximized: false,
        has_focus: true,
        event_callbacks: HashMap::new(),
    }));
    TAURI_WINDOWS.lock().unwrap().insert(this, win);
}

/// Enregistre un handler d'événement JS → Ocara
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_listen(this: i64, event_ptr: i64, callback_ptr: i64) {
    let event = unsafe { ptr_to_str(event_ptr).to_string() };
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().event_callbacks.insert(event, callback_ptr);
    }
}

/// Émet un événement Ocara → JS
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_emit(this: i64, event_ptr: i64, data_ptr: i64) {
    let event = unsafe { ptr_to_str(event_ptr as i64).to_string() };
    let data = unsafe { ptr_to_str(data_ptr as i64).to_string() };
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        // Réservé pour un futur état à mettre à jour (dernier événement émis, etc.)
        let _w = win.lock().unwrap();
        // Ici, on pourrait appeler l'API Tauri réelle
        println!("[Tauri_emit] {}: {}", event, data);
    }
}

/// Ouvre une boîte de dialogue native
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_dialog(this: i64, options_ptr: i64) -> i64 {
    let options = unsafe { ptr_to_str(options_ptr as i64).to_string() };
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(_win) = map.get(&this) {
        // Simule une réponse utilisateur
        let response = "ok".to_string();
        println!("[Tauri_dialog] options: {} -> {}", options, response);
        unsafe { alloc_str(&response) }
    } else {
        0
    }
}

/// Envoie une notification système
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_notify(this: i64, options_ptr: i64) {
    let options = unsafe { ptr_to_str(options_ptr as i64).to_string() };
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(_win) = map.get(&this) {
        println!("[Tauri_notify] options: {}", options);
    }
}

// ─────────────────────────────────────────────────────────────
// Méthodes d'accès et gestion de fenêtre (parité avec autres builtins)
// ─────────────────────────────────────────────────────────────

/// Récupère le titre de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getTitle(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let title = win.lock().unwrap().title.clone();
        unsafe { alloc_str(&title) }
    } else {
        0
    }
}

/// Définit le titre de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setTitle(this: i64, title_ptr: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let title = unsafe { ptr_to_str(title_ptr).to_string() };
        win.lock().unwrap().title = title;
    }
}

/// Récupère la largeur de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getWidth(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().width
    } else {
        800
    }
}

/// Définit la largeur de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setWidth(this: i64, width: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().width = width;
    }
}

/// Récupère la hauteur de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getHeight(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().height
    } else {
        600
    }
}

/// Définit la hauteur de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setHeight(this: i64, height: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        win.lock().unwrap().height = height;
    }
}

/// Récupère l'URL courante
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_getUrl(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let url = win.lock().unwrap().url.clone();
        unsafe { alloc_str(&url) }
    } else {
        0
    }
}

/// Définit l'URL de la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_setUrl(this: i64, url_ptr: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let url = unsafe { ptr_to_str(url_ptr).to_string() };
        win.lock().unwrap().url = url;
    }
}

/// Ouvre la fenêtre (si minimisée/fermée)
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_open(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_open = true;
        w.is_minimized = false;
    }
}

/// Ferme la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_close(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_open = false;
    }
}

/// Vérifie si la fenêtre est ouverte
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_isOpen(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        if win.lock().unwrap().is_open { 1 } else { 0 }
    } else {
        0
    }
}

/// Met la fenêtre au premier plan
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_focus(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.has_focus = true;
    }
}

/// Minimise la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_minimize(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_minimized = true;
        w.is_maximized = false;
    }
}

/// Maximiser la fenêtre
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_maximize(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_maximized = true;
        w.is_minimized = false;
    }
}

/// Restaure la fenêtre (si minimisée/maximisée)
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_restore(this: i64) {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        let mut w = win.lock().unwrap();
        w.is_minimized = false;
        w.is_maximized = false;
    }
}

/// Vérifie si la fenêtre a le focus
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_hasFocus(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        if win.lock().unwrap().has_focus { 1 } else { 0 }
    } else {
        0
    }
}

/// Vérifie si la fenêtre est minimisée
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_isMinimized(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        if win.lock().unwrap().is_minimized { 1 } else { 0 }
    } else {
        0
    }
}

/// Vérifie si la fenêtre est maximisée
#[unsafe(no_mangle)]
pub extern "C" fn Tauri_isMaximized(this: i64) -> i64 {
    let map = TAURI_WINDOWS.lock().unwrap();
    if let Some(win) = map.get(&this) {
        if win.lock().unwrap().is_maximized { 1 } else { 0 }
    } else {
        0
    }
}
