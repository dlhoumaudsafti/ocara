use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

pub const LOWLEVEL_BUILTINS: &[BuiltinDesc] = &[
    // ── Internes (toujours disponibles, jamais appelables directement) ────────
    BuiltinDesc { name: "__str_concat",      params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__val_to_str",      params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__str_from_float",  params: &[clt::F64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__str_from_bool",   params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__box_float",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__int_to_float",    params: &[clt::I64],                             returns: Some(clt::F64),    module: None },
    BuiltinDesc { name: "__box_bool",        params: &[clt::I64],                             returns: Some(clt::I64),    module: None },

    // ── Type checking runtime (narrowing 'is Type') ───────────────────────────
    BuiltinDesc { name: "__is_null",         params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__is_int",          params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__is_float",        params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__is_bool",         params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__is_string",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__is_array",        params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__is_map",          params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__is_object",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__is_function",     params: &[clt::I64],                             returns: Some(clt::I64),    module: None },

    BuiltinDesc { name: "__range",           params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_new",       params: &[],                                     returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_push",      params: &[clt::I64, clt::I64],                   returns: None,              module: None },
    BuiltinDesc { name: "__array_len",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_get",       params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_set",       params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: None },
    BuiltinDesc { name: "__map_foreach",     params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: None },
    BuiltinDesc { name: "__map_new",         params: &[],                                     returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__map_set",         params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: None },
    BuiltinDesc { name: "__map_get",         params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },

    // ── Libération / clonage (scoped/consumed — voir docs/EBNF.md) ────────────
    // `__value_free`/`__value_clone` dispatchent sur le tag RUNTIME (pas le
    // type statique AST) — seul moyen sûr de savoir si une `string` donnée
    // est réellement possédée (tas) ou empruntée (littéral en .rodata).
    // Point d'entrée unique utilisé par le lowering pour toute `scoped`/
    // `consumed` de type valeur (string/array/map) — voir
    // crate::lower::stmt::ownership.
    BuiltinDesc { name: "__value_free",      params: &[clt::I64],                             returns: None,               module: None },
    BuiltinDesc { name: "__value_clone",     params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_free",      params: &[clt::I64],                             returns: None,               module: None },
    BuiltinDesc { name: "__map_free",        params: &[clt::I64],                             returns: None,               module: None },
    BuiltinDesc { name: "__array_clone",     params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__map_clone",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },

    // ── Comparaisons avec vérification de type au runtime ─────────────────────
    // Utilisées uniquement quand sema n'a pas pu vérifier statiquement (au
    // moins un opérande `mixed`) — sinon la comparaison est émise en direct
    // (CmpEq/CmpLt/...). Voir equal/not equal/smaller/greater/
    // smaller or equal/greater or equal dans lower::expr::lower.
    BuiltinDesc { name: "__cmp_eq_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__cmp_ne_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__cmp_lt_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__cmp_gt_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__cmp_le_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__cmp_ge_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    
    // ── Gestion des erreurs (try/on/fail) — toujours disponibles ─────────────
    BuiltinDesc { name: "__ocara_try_exec",             params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_try_exec_with_captures", params: &[clt::I64, clt::I64, clt::I64],    returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_handler_set_return",   params: &[clt::I64],                        returns: None,              module: None },
    BuiltinDesc { name: "__ocara_fail",                 params: &[clt::I64, clt::I64],              returns: None,              module: None },
    BuiltinDesc { name: "__ocara_type_matches",         params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__task_spawn",           params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__task_resolve",         params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__unbox_float",          params: &[clt::I64],                        returns: Some(clt::F64),    module: None },
    BuiltinDesc { name: "__unbox_bool",           params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_unhandled_fail", params: &[clt::I64],                        returns: None,              module: None },
    
    // Allocation d'objet tas (toujours disponible)
    BuiltinDesc { name: "__alloc_obj",            params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__alloc_class_obj",      params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__alloc_fat_ptr",        params: &[],                                returns: Some(clt::I64),    module: None },
    
    // Conversion string — sans heuristique pointeur
    BuiltinDesc { name: "__str_from_int",         params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_to_str",         params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__system_os",            params: &[],                                returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__system_arch",          params: &[],                                returns: Some(clt::I64),    module: None },

    // Trampolines ui.handler / ui.handlers (voir src/lower/expr.d/tauri_handler.rs)
    BuiltinDesc { name: "__tauri_obj_is_string",   params: &[clt::I64, clt::I64],  returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_obj_is_int",      params: &[clt::I64, clt::I64],  returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_obj_is_float",    params: &[clt::I64, clt::I64],  returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_obj_is_bool",     params: &[clt::I64, clt::I64],  returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_obj_get_string",  params: &[clt::I64, clt::I64],  returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_obj_get_int",     params: &[clt::I64, clt::I64],  returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_obj_get_float",   params: &[clt::I64, clt::I64],  returns: Some(clt::F64), module: None },
    BuiltinDesc { name: "__tauri_obj_get_bool",    params: &[clt::I64, clt::I64],  returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_ok_string",       params: &[clt::I64],            returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_ok_int",          params: &[clt::I64],            returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_ok_float",        params: &[clt::F64],            returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_ok_bool",         params: &[clt::I64],            returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_ok_void",         params: &[],                    returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "__tauri_err",             params: &[clt::I64],            returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "Tauri_handler_register",  params: &[clt::I64, clt::I64, clt::I64, clt::I64], returns: None, module: None },
];
