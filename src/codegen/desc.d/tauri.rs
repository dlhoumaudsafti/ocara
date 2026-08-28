// src/codegen/desc.d/tauri.rs — signatures Cranelift pour builtin Tauri

use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

pub const TAURI_BUILTINS: &[BuiltinDesc] = &[
// Constructeur (use Tauri) — le codegen appelle toujours `<Classe>_init(this, ...)`
// pour `use Classe(...)` (voir Expr::New dans src/lower/expr.d/lower.rs), le
// pointeur alloué (`this`) devient la valeur de l'objet côté Ocara : c'est LUI qui
// sert ensuite de handle pour tous les appels d'instance, pas une valeur de retour.
	BuiltinDesc {
		name: "Tauri_init",
		params: &[clt::I64, clt::I64], // this, options: map<string, mixed> (pointeur)
		returns: None,
		module: Some("Tauri"),
	},
// Méthodes d'instance
	BuiltinDesc {
		name: "Tauri_listen",
		params: &[clt::I64, clt::I64, clt::I64], // this, event, callback
		returns: None,
		module: Some("Tauri"),
	},
	BuiltinDesc {
		name: "Tauri_emit",
		params: &[clt::I64, clt::I64, clt::I64], // this, event, data
		returns: None,
		module: Some("Tauri"),
	},
	BuiltinDesc {
		name: "Tauri_dialog",
		params: &[clt::I64, clt::I64], // this, options
		returns: Some(clt::I64), // string
		module: Some("Tauri"),
	},
	BuiltinDesc {
		name: "Tauri_notify",
		params: &[clt::I64, clt::I64], // this, options
		returns: None,
		module: Some("Tauri"),
	},
// Getters/setters et gestion fenêtre (parité runtime)
	BuiltinDesc {
		name:    "Tauri_getTitle",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_setTitle",
		params:  &[clt::I64, clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_getWidth",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_setWidth",
		params:  &[clt::I64, clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_getHeight",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_setHeight",
		params:  &[clt::I64, clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_getUrl",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_setUrl",
		params:  &[clt::I64, clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_open",
		params:  &[clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_close",
		params:  &[clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_isOpen",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_focus",
		params:  &[clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_minimize",
		params:  &[clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_maximize",
		params:  &[clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_restore",
		params:  &[clt::I64],
		returns: None,
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_hasFocus",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_isMinimized",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("Tauri"),
	},
	BuiltinDesc {
		name:    "Tauri_isMaximized",
		params:  &[clt::I64],
		returns: Some(clt::I64),
		module:  Some("Tauri"),
	},
];
