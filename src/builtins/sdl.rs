// ─────────────────────────────────────────────────────────────────────────────
// ocara.SDL — classe builtin pour fenêtrage + rendu 2D + entrées (SDL3)
//
// Palier 1 (MVP) : une fenêtre + un renderer 2D + une pompe d'événements
// bundlés dans un seul objet. Palier 2 : textures/images (SDL_image) et
// fonts/texte (SDL_ttf). Palier 3 : manettes (SDL_gamepad) et audio
// (SDL_mixer), toujours sur ce même objet.
//
// Méthodes d'instance :
//   use SDL(options: map<string, mixed>) → SDL   // {title, width, height}
//   sdl.pollEvent() → map<string, mixed>          // {"type":"none"} si vide
//   sdl.setDrawColor(r, g, b, a) / clear() / present()
//   sdl.fillRect/drawRect(x, y, w, h) / drawLine(x1,y1,x2,y2) / drawPoint(x,y)
//   sdl.isOpen() / close() / getWidth() / getHeight() / getTitle() / setTitle()
//   sdl.loadTexture(path) → int (handle) / textureWidth/Height(id) → int
//   sdl.drawTexture(id, x, y) / drawTextureScaled(id, x, y, w, h)
//   sdl.loadFont(path, size) → int (handle) / drawText(id, text, x, y, r,g,b,a)
// Méthodes statiques :
//   SDL::ticks() → int
//   SDL::delay(ms: int)
//
// Convention runtime : SDL_<method>
// Contrainte : une seule fenêtre SDL par processus (Sdl::event_pump() ne peut
// être appelé qu'une fois), et tous les appels doivent venir du thread qui a
// créé la fenêtre — voir docs/builtins/SDL.md.
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn static_m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
    }
}

fn inst_m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: false,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
    }
}

pub fn sdl_class() -> ClassInfo {
    let mut methods = HashMap::new();

    // Constructeur (use SDL) — crée fenêtre + renderer + pompe d'événements.
    methods.insert(
        "__init__".to_string(),
        static_m(vec![ ("options", Type::Map(Box::new(Type::String), Box::new(Type::Mixed))) ], Type::Named("SDL".to_string())),
    );

    // pollEvent — vide la file un événement à la fois ; {"type":"none"} quand elle est vide.
    methods.insert(
        "pollEvent".to_string(),
        inst_m(vec![], Type::Map(Box::new(Type::String), Box::new(Type::Mixed))),
    );

    // Dessin
    methods.insert("setDrawColor".to_string(), inst_m(
        vec![ ("r", Type::Int), ("g", Type::Int), ("b", Type::Int), ("a", Type::Int) ],
        Type::Void,
    ));
    methods.insert("clear".to_string(), inst_m(vec![], Type::Void));
    methods.insert("fillRect".to_string(), inst_m(
        vec![ ("x", Type::Int), ("y", Type::Int), ("w", Type::Int), ("h", Type::Int) ],
        Type::Void,
    ));
    methods.insert("drawRect".to_string(), inst_m(
        vec![ ("x", Type::Int), ("y", Type::Int), ("w", Type::Int), ("h", Type::Int) ],
        Type::Void,
    ));
    methods.insert("drawLine".to_string(), inst_m(
        vec![ ("x1", Type::Int), ("y1", Type::Int), ("x2", Type::Int), ("y2", Type::Int) ],
        Type::Void,
    ));
    methods.insert("drawPoint".to_string(), inst_m(
        vec![ ("x", Type::Int), ("y", Type::Int) ],
        Type::Void,
    ));
    methods.insert("present".to_string(), inst_m(vec![], Type::Void));

    // Fenêtre : état + getters/setters (requête live, pas de cache — cf. Tauri)
    methods.insert("isOpen".to_string(), inst_m(vec![], Type::Bool));
    methods.insert("close".to_string(), inst_m(vec![], Type::Void));
    methods.insert("getWidth".to_string(), inst_m(vec![], Type::Int));
    methods.insert("getHeight".to_string(), inst_m(vec![], Type::Int));
    methods.insert("getTitle".to_string(), inst_m(vec![], Type::String));
    methods.insert("setTitle".to_string(), inst_m(vec![ ("title", Type::String) ], Type::Void));

    // Timing (statique — indépendant de toute instance de fenêtre)
    methods.insert("ticks".to_string(), static_m(vec![], Type::Int));
    methods.insert("delay".to_string(), static_m(vec![ ("ms", Type::Int) ], Type::Void));

    // ── Palier 2 : textures/images (SDL_image) ─────────────────────────────
    methods.insert("loadTexture".to_string(), inst_m(
        vec![ ("path", Type::String) ], Type::Int, // handle
    ));
    methods.insert("textureWidth".to_string(), inst_m(vec![ ("textureId", Type::Int) ], Type::Int));
    methods.insert("textureHeight".to_string(), inst_m(vec![ ("textureId", Type::Int) ], Type::Int));
    methods.insert("drawTexture".to_string(), inst_m(
        vec![ ("textureId", Type::Int), ("x", Type::Int), ("y", Type::Int) ],
        Type::Void,
    ));
    methods.insert("drawTextureScaled".to_string(), inst_m(
        vec![ ("textureId", Type::Int), ("x", Type::Int), ("y", Type::Int), ("w", Type::Int), ("h", Type::Int) ],
        Type::Void,
    ));

    // ── Palier 2 : fonts/texte (SDL_ttf) ────────────────────────────────────
    methods.insert("loadFont".to_string(), inst_m(
        vec![ ("path", Type::String), ("size", Type::Int) ], Type::Int, // handle
    ));
    methods.insert("drawText".to_string(), inst_m(
        vec![
            ("fontId", Type::Int), ("text", Type::String),
            ("x", Type::Int), ("y", Type::Int),
            ("r", Type::Int), ("g", Type::Int), ("b", Type::Int), ("a", Type::Int),
        ],
        Type::Void,
    ));

    // ── Palier 3 : manettes (SDL_gamepad) ───────────────────────────────────
    // Connexion/déconnexion/boutons/axes arrivent via pollEvent() (types
    // "gamepadconnected"/"gamepaddisconnected"/"gamepadbuttondown"/
    // "gamepadbuttonup"/"gamepadaxis") — pas de nouvelles méthodes pour ça.
    // Ici : uniquement l'état direct (utile pour un mouvement continu, ex.
    // stick analogique → vitesse du joueur, chaque frame).
    methods.insert("isButtonPressed".to_string(), inst_m(
        vec![ ("gamepadId", Type::Int), ("button", Type::String) ], Type::Bool,
    ));
    methods.insert("getAxis".to_string(), inst_m(
        vec![ ("gamepadId", Type::Int), ("axis", Type::String) ], Type::Int,
    ));

    // ── Palier 3 : audio (SDL_mixer) ────────────────────────────────────────
    methods.insert("loadSound".to_string(), inst_m(
        vec![ ("path", Type::String) ], Type::Int, // handle
    ));
    methods.insert("playSound".to_string(), inst_m(vec![ ("soundId", Type::Int) ], Type::Void));
    methods.insert("playMusic".to_string(), inst_m(
        vec![ ("path", Type::String), ("loop", Type::Bool) ], Type::Void,
    ));
    methods.insert("pauseMusic".to_string(), inst_m(vec![], Type::Void));
    methods.insert("resumeMusic".to_string(), inst_m(vec![], Type::Void));
    methods.insert("stopMusic".to_string(), inst_m(vec![], Type::Void));
    methods.insert("setMusicVolume".to_string(), inst_m(vec![ ("volume", Type::Int) ], Type::Void));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
