// src/codegen/desc.d/sdl.rs — signatures Cranelift pour builtin SDL

use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

pub const SDL_BUILTINS: &[BuiltinDesc] = &[
// Constructeur (use SDL) — le codegen appelle toujours `<Classe>_init(this, ...)`
// pour `use Classe(...)` (voir Expr::New dans src/lower/expr.d/lower.rs), le
// pointeur alloué (`this`) devient la valeur de l'objet côté Ocara : c'est LUI qui
// sert ensuite de handle pour tous les appels d'instance, pas une valeur de retour.
	BuiltinDesc {
		name: "SDL_init",
		params: &[clt::I64, clt::I64], // this, options: map<string, mixed> (pointeur)
		returns: None,
		module: Some("SDL"),
	},
// Événements
	BuiltinDesc {
		name:    "SDL_pollEvent",
		params:  &[clt::I64],   // this
		returns: Some(clt::I64), // map<string, mixed>
		module:  Some("SDL"),
	},
// Dessin
	BuiltinDesc {
		name:    "SDL_setDrawColor",
		params:  &[clt::I64, clt::I64, clt::I64, clt::I64, clt::I64], // this, r, g, b, a
		returns: None,
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_clear",
		params:  &[clt::I64], // this
		returns: None,
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_fillRect",
		params:  &[clt::I64, clt::I64, clt::I64, clt::I64, clt::I64], // this, x, y, w, h
		returns: None,
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_drawRect",
		params:  &[clt::I64, clt::I64, clt::I64, clt::I64, clt::I64], // this, x, y, w, h
		returns: None,
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_drawLine",
		params:  &[clt::I64, clt::I64, clt::I64, clt::I64, clt::I64], // this, x1, y1, x2, y2
		returns: None,
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_drawPoint",
		params:  &[clt::I64, clt::I64, clt::I64], // this, x, y
		returns: None,
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_present",
		params:  &[clt::I64], // this
		returns: None,
		module:  Some("SDL"),
	},
// Fenêtre : état + getters/setters
	BuiltinDesc {
		name:    "SDL_isOpen",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_close",
		params:  &[clt::I64],
		returns: None,
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_getWidth",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_getHeight",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_getTitle",
		params:  &[clt::I64],
		returns: Some(clt::I64), // string
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_setTitle",
		params:  &[clt::I64, clt::I64], // this, title
		returns: None,
		module:  Some("SDL"),
	},
// Timing (statique — pas de `this`)
	BuiltinDesc {
		name:    "SDL_ticks",
		params:  &[],
		returns: Some(clt::I64),
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_delay",
		params:  &[clt::I64], // ms
		returns: None,
		module:  Some("SDL"),
	},
// Palier 2 : textures/images
	BuiltinDesc {
		name:    "SDL_loadTexture",
		params:  &[clt::I64, clt::I64], // this, path
		returns: Some(clt::I64),        // handle (int)
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_textureWidth",
		params:  &[clt::I64, clt::I64], // this, textureId
		returns: Some(clt::I64),
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_textureHeight",
		params:  &[clt::I64, clt::I64], // this, textureId
		returns: Some(clt::I64),
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_drawTexture",
		params:  &[clt::I64, clt::I64, clt::I64, clt::I64], // this, textureId, x, y
		returns: None,
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_drawTextureScaled",
		params:  &[clt::I64, clt::I64, clt::I64, clt::I64, clt::I64, clt::I64], // this, textureId, x, y, w, h
		returns: None,
		module:  Some("SDL"),
	},
// Palier 2 : fonts/texte
	BuiltinDesc {
		name:    "SDL_loadFont",
		params:  &[clt::I64, clt::I64, clt::I64], // this, path, size
		returns: Some(clt::I64),                  // handle (int)
		module:  Some("SDL"),
	},
	BuiltinDesc {
		name:    "SDL_drawText",
		params:  &[
			clt::I64, clt::I64, clt::I64, // this, fontId, text
			clt::I64, clt::I64,           // x, y
			clt::I64, clt::I64, clt::I64, clt::I64, // r, g, b, a
		],
		returns: None,
		module:  Some("SDL"),
	},
];
