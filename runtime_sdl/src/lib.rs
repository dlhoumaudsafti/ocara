// runtime_sdl/src/lib.rs — intégration SDL3 native, crate séparé de
// ocara_runtime (voir la doc dans runtime_sdl/Cargo.toml et
// src/codegen/link.rs : c'est ce qui permet de ne lier SDL3 que pour les
// programmes qui importent réellement ocara.SDL).
//
// Palier 1 (MVP) : une fenêtre + un renderer 2D + une pompe d'événements
// bundlés dans un seul objet Ocara. Deux contraintes structurelles (SDL, pas
// des choix Ocara) :
//   - Une seule fenêtre par processus : `Sdl::event_pump()` échoue si appelé
//     une 2e fois — SDL_ACTIVE empêche un 2e `use SDL(...)` de se propager
//     comme un panic natif, on lève SDLException proprement à la place.
//   - Tous les appels sur une fenêtre doivent venir du thread qui l'a créée
//     (SDL/l'OS l'exigent, plus strictement encore sur macOS/Cocoa) —
//     `owner_thread` fait respecter ça, voir la note SAFETY plus bas.
// Voir docs/builtins/SDL.md.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;

use once_cell::sync::Lazy;

use ocara_runtime::{__map_get, __map_new, __map_set, alloc_str, ptr_to_str};

use sdl3::event::{Event, WindowEvent};
use sdl3::pixels::Color;
use sdl3::render::{Canvas, FPoint, FRect};
use sdl3::video::Window;
use sdl3::{EventPump, Sdl};

/// Convertit n'importe quelle erreur SDL (Display ou pas) en message lisible
/// pour SDLException — Debug est quasi universellement dérivé, contrairement
/// à Display qui n'est pas garanti sur tous les types d'erreur de sdl3-rs.
fn err_string<E: std::fmt::Debug>(e: E) -> String {
    format!("{:?}", e)
}

/// Une fenêtre SDL côté Ocara : contexte + canvas (renderer) + pompe
/// d'événements, plus le thread qui l'a créée.
struct OcaraSdlWindow {
    _sdl:         Sdl, // gardé en vie tant que la fenêtre existe (RAII)
    canvas:       Canvas<Window>,
    event_pump:   EventPump,
    owner_thread: ThreadId,
    is_open:      bool,
}

// SAFETY: Sdl/Window/Canvas/EventPump ne sont PAS Send dans sdl3-rs — ils
// enveloppent des pointeurs bruts et SDL exige une affinité de thread stricte.
// Ce wrapper permet à la structure de vivre dans le registre statique
// ci-dessous, exactement comme runtime_tauri le fait avec ses fenêtres GTK
// (Arc<Mutex<...>>) — il ne rend PAS les appels cross-thread sûrs : chaque
// fonction exportée vérifie `owner_thread` (check_owner_thread) et lève
// SDLException plutôt que de toucher l'état SDL depuis le mauvais thread.
unsafe impl Send for OcaraSdlWindow {}

/// Table globale des fenêtres créées (clé = pointeur Ocara `this`), même
/// convention que TAURI_WINDOWS dans runtime_tauri.
static SDL_WINDOWS: Lazy<Mutex<HashMap<i64, Arc<Mutex<OcaraSdlWindow>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Voir la doc de module : une seule fenêtre SDL par processus en Palier 1.
static SDL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Lit une clé string d'une map d'options (`__map_get` + décodage du pointeur
/// string), ou renvoie `default` si la clé est absente.
unsafe fn map_get_str(options_ptr: i64, key: &str, default: &str) -> String {
    let key_ptr = unsafe { alloc_str(key) };
    let val = __map_get(options_ptr, key_ptr);
    if val == 0 {
        default.to_string()
    } else {
        unsafe { ptr_to_str(val).to_string() }
    }
}

/// Lit une clé int d'une map d'options, ou renvoie `default` si absente.
unsafe fn map_get_int(options_ptr: i64, key: &str, default: i64) -> i64 {
    let key_ptr = unsafe { alloc_str(key) };
    let val = __map_get(options_ptr, key_ptr);
    if val == 0 { default } else { val }
}

/// Valeur d'un champ à écrire dans une map<string,mixed> de retour.
enum MixedVal<'a> {
    Str(&'a str),
    Int(i64),
}

/// Construit une map<string,mixed> Ocara depuis une liste de paires
/// (clé, valeur) — même primitives runtime (`__map_new`/`__map_set`) que
/// celles déjà utilisées par SQLite/MySQL/YAML pour renvoyer une map à Ocara.
fn build_map(pairs: &[(&str, MixedVal)]) -> i64 {
    let m = __map_new();
    for (k, v) in pairs {
        let key_ptr = unsafe { alloc_str(k) };
        let val = match v {
            MixedVal::Str(s) => unsafe { alloc_str(s) },
            MixedVal::Int(n) => *n,
        };
        __map_set(m, key_ptr, val);
    }
    m
}

/// Vérifie que l'appel courant vient bien du thread qui a créé la fenêtre ;
/// lève SDLException sinon (SDL/l'OS ne tolèrent pas un appel cross-thread).
fn check_owner_thread(win: &OcaraSdlWindow) {
    if win.owner_thread != std::thread::current().id() {
        unsafe {
            ocara_runtime::exception::throw_sdl_exception(
                "SDL doit être utilisé uniquement depuis le thread qui a créé la fenêtre (voir docs/builtins/SDL.md)",
                201,
            );
        }
    }
}

/// Récupère la fenêtre `this`, vérifie l'affinité de thread et qu'elle est
/// encore ouverte, puis exécute `f` dessus. No-op silencieux si la fenêtre
/// n'existe pas ou a été fermée (comportement volontairement permissif —
/// Palier 1 ne distingue pas "jamais ouverte" de "fermée" côté appelant).
fn with_open_window(this: i64, f: impl FnOnce(&mut OcaraSdlWindow)) {
    let win_arc = {
        let map = SDL_WINDOWS.lock().unwrap();
        match map.get(&this) {
            Some(w) => w.clone(),
            None => return,
        }
    };
    let mut win = win_arc.lock().unwrap();
    check_owner_thread(&win);
    if !win.is_open {
        return;
    }
    f(&mut win);
}

/// Crée la fenêtre + le renderer + la pompe d'événements. `this` est le
/// pointeur alloué par `use SDL(...)` (voir Expr::New dans
/// src/lower/expr.d/lower.rs, qui appelle systématiquement `<Classe>_init(this, ...)` —
/// c'est pourquoi ce constructeur doit s'appeler `SDL_init`, pas `SDL___init__`).
/// `this` sert ensuite de clé/handle pour tous les appels d'instance suivants.
#[unsafe(no_mangle)]
pub extern "C" fn SDL_init(this: i64, options_ptr: i64) {
    if SDL_ACTIVE.swap(true, Ordering::SeqCst) {
        unsafe {
            ocara_runtime::exception::throw_sdl_exception(
                "une fenêtre SDL est déjà ouverte dans ce processus (une seule à la fois en Palier 1)",
                101,
            );
        }
    }

    let title  = unsafe { map_get_str(options_ptr, "title", "Ocara App") };
    let width  = unsafe { map_get_int(options_ptr, "width", 800) };
    let height = unsafe { map_get_int(options_ptr, "height", 600) };

    let result = (|| -> Result<OcaraSdlWindow, String> {
        let sdl_context = sdl3::init().map_err(err_string)?;
        let video = sdl_context.video().map_err(err_string)?;
        let window = video
            .window(&title, width.max(1) as u32, height.max(1) as u32)
            .build()
            .map_err(err_string)?;
        let canvas = window.into_canvas();
        let event_pump = sdl_context.event_pump().map_err(err_string)?;
        Ok(OcaraSdlWindow {
            _sdl: sdl_context,
            canvas,
            event_pump,
            owner_thread: std::thread::current().id(),
            is_open: true,
        })
    })();

    match result {
        Ok(win) => {
            SDL_WINDOWS.lock().unwrap().insert(this, Arc::new(Mutex::new(win)));
        }
        Err(msg) => {
            // Init ratée : on libère SDL_ACTIVE pour ne pas bloquer une
            // éventuelle nouvelle tentative après un `try`/`on` côté Ocara.
            SDL_ACTIVE.store(false, Ordering::SeqCst);
            unsafe {
                ocara_runtime::exception::throw_sdl_exception(
                    &format!("échec d'initialisation SDL: {}", msg),
                    102,
                );
            }
        }
    }
}

/// Vide la file d'événements un par un ; `{"type":"none"}` quand elle est
/// vide. Note : la fermeture par la croix de la fenêtre arrive comme
/// `Event::Window{win_event: CloseRequested}` en SDL3, PAS `Event::Quit`
/// (réservé à un quit niveau OS/session) — les deux sont mappés vers "quit".
#[unsafe(no_mangle)]
pub extern "C" fn SDL_pollEvent(this: i64) -> i64 {
    let win_arc = {
        let map = SDL_WINDOWS.lock().unwrap();
        match map.get(&this) {
            Some(w) => w.clone(),
            None => return build_map(&[("type", MixedVal::Str("none"))]),
        }
    };
    let mut win = win_arc.lock().unwrap();
    check_owner_thread(&win);

    match win.event_pump.poll_event() {
        None => build_map(&[("type", MixedVal::Str("none"))]),

        Some(Event::Quit { .. }) => build_map(&[("type", MixedVal::Str("quit"))]),
        Some(Event::Window { win_event: WindowEvent::CloseRequested, .. }) => {
            build_map(&[("type", MixedVal::Str("quit"))])
        }
        Some(Event::Window { win_event: WindowEvent::Resized(w, h), .. }) => build_map(&[
            ("type", MixedVal::Str("resize")),
            ("width", MixedVal::Int(w as i64)),
            ("height", MixedVal::Int(h as i64)),
        ]),

        Some(Event::KeyDown { keycode, repeat, .. }) => {
            let name = keycode.map(|k| k.name()).unwrap_or_default();
            build_map(&[
                ("type", MixedVal::Str("keydown")),
                ("key", MixedVal::Str(&name)),
                ("repeat", MixedVal::Int(if repeat { 1 } else { 0 })),
            ])
        }
        Some(Event::KeyUp { keycode, repeat, .. }) => {
            let name = keycode.map(|k| k.name()).unwrap_or_default();
            build_map(&[
                ("type", MixedVal::Str("keyup")),
                ("key", MixedVal::Str(&name)),
                ("repeat", MixedVal::Int(if repeat { 1 } else { 0 })),
            ])
        }

        Some(Event::MouseMotion { x, y, xrel, yrel, .. }) => build_map(&[
            ("type", MixedVal::Str("mousemotion")),
            ("x", MixedVal::Int(x as i64)),
            ("y", MixedVal::Int(y as i64)),
            ("xrel", MixedVal::Int(xrel as i64)),
            ("yrel", MixedVal::Int(yrel as i64)),
        ]),
        Some(Event::MouseButtonDown { mouse_btn, x, y, .. }) => build_map(&[
            ("type", MixedVal::Str("mousebuttondown")),
            ("button", MixedVal::Int(mouse_btn as i64)),
            ("x", MixedVal::Int(x as i64)),
            ("y", MixedVal::Int(y as i64)),
        ]),
        Some(Event::MouseButtonUp { mouse_btn, x, y, .. }) => build_map(&[
            ("type", MixedVal::Str("mousebuttonup")),
            ("button", MixedVal::Int(mouse_btn as i64)),
            ("x", MixedVal::Int(x as i64)),
            ("y", MixedVal::Int(y as i64)),
        ]),
        Some(Event::MouseWheel { integer_x, integer_y, .. }) => build_map(&[
            ("type", MixedVal::Str("mousewheel")),
            ("x", MixedVal::Int(integer_x as i64)),
            ("y", MixedVal::Int(integer_y as i64)),
        ]),

        // Tout autre événement (autres sous-types Window, joystick, etc. —
        // hors périmètre Palier 1) : passthrough générique plutôt que perdu.
        Some(_) => build_map(&[("type", MixedVal::Str("unknown"))]),
    }
}

// ── Dessin ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn SDL_setDrawColor(this: i64, r: i64, g: i64, b: i64, a: i64) {
    with_open_window(this, |win| {
        win.canvas.set_draw_color(Color::RGBA(r as u8, g as u8, b as u8, a as u8));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_clear(this: i64) {
    with_open_window(this, |win| win.canvas.clear());
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_fillRect(this: i64, x: i64, y: i64, w: i64, h: i64) {
    with_open_window(this, |win| {
        let rect = FRect::new(x as f32, y as f32, w as f32, h as f32);
        let _ = win.canvas.fill_rect(Some(rect));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_drawRect(this: i64, x: i64, y: i64, w: i64, h: i64) {
    with_open_window(this, |win| {
        let rect = FRect::new(x as f32, y as f32, w as f32, h as f32);
        let _ = win.canvas.draw_rect(rect);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_drawLine(this: i64, x1: i64, y1: i64, x2: i64, y2: i64) {
    with_open_window(this, |win| {
        let p1 = FPoint::new(x1 as f32, y1 as f32);
        let p2 = FPoint::new(x2 as f32, y2 as f32);
        let _ = win.canvas.draw_line(p1, p2);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_drawPoint(this: i64, x: i64, y: i64) {
    with_open_window(this, |win| {
        let p = FPoint::new(x as f32, y as f32);
        let _ = win.canvas.draw_point(p);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_present(this: i64) {
    with_open_window(this, |win| {
        let _ = win.canvas.present();
    });
}

// ── Fenêtre : état + getters/setters (requête live, pas de cache) ──────────

#[unsafe(no_mangle)]
pub extern "C" fn SDL_isOpen(this: i64) -> i64 {
    let map = SDL_WINDOWS.lock().unwrap();
    match map.get(&this) {
        Some(w) => if w.lock().unwrap().is_open { 1 } else { 0 },
        None => 0,
    }
}

/// Ne détruit PAS la fenêtre SDL sous-jacente (Palier 1 : garder ça simple —
/// le process gardera la fenêtre en mémoire jusqu'à sa terminaison). Marque
/// juste `is_open = false` côté Ocara : `isOpen()` reflète l'intention de
/// l'appelant, `with_open_window` ignore silencieusement les appels suivants.
/// Ré-ouvrir une nouvelle fenêtre dans le même process n'est PAS supporté en
/// Palier 1 (voir SDL_ACTIVE / docs/builtins/SDL.md).
#[unsafe(no_mangle)]
pub extern "C" fn SDL_close(this: i64) {
    let map = SDL_WINDOWS.lock().unwrap();
    if let Some(w) = map.get(&this) {
        w.lock().unwrap().is_open = false;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_getWidth(this: i64) -> i64 {
    let map = SDL_WINDOWS.lock().unwrap();
    match map.get(&this) {
        Some(w) => {
            let win = w.lock().unwrap();
            check_owner_thread(&win);
            win.canvas.window().size().0 as i64
        }
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_getHeight(this: i64) -> i64 {
    let map = SDL_WINDOWS.lock().unwrap();
    match map.get(&this) {
        Some(w) => {
            let win = w.lock().unwrap();
            check_owner_thread(&win);
            win.canvas.window().size().1 as i64
        }
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_getTitle(this: i64) -> i64 {
    let map = SDL_WINDOWS.lock().unwrap();
    match map.get(&this) {
        Some(w) => {
            let win = w.lock().unwrap();
            check_owner_thread(&win);
            unsafe { alloc_str(win.canvas.window().title()) }
        }
        None => unsafe { alloc_str("") },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_setTitle(this: i64, title_ptr: i64) {
    let title = unsafe { ptr_to_str(title_ptr).to_string() };
    with_open_window(this, |win| {
        let _ = win.canvas.window_mut().set_title(&title);
    });
}

// ── Timing (statique — pas de fenêtre requise) ──────────────────────────────
//
// Limite Palier 1 : SDL doit avoir été initialisé (un `use SDL(...)` doit
// avoir eu lieu) avant d'appeler ces deux fonctions statiques — les appeler
// avant toute création de fenêtre n'est pas garanti fonctionner.

#[unsafe(no_mangle)]
pub extern "C" fn SDL_ticks() -> i64 {
    sdl3::timer::ticks() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn SDL_delay(ms: i64) {
    sdl3::timer::delay(ms.max(0) as u32);
}
