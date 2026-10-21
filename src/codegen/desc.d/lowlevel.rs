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
    BuiltinDesc { name: "__box_int_for_mixed", params: &[clt::I64],                           returns: Some(clt::I64),    module: None },
    // Division et modulo entiers gardés (diviseur nul → MathException)
    BuiltinDesc { name: "__int_div",           params: &[clt::I64, clt::I64],                 returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__int_mod",           params: &[clt::I64, clt::I64],                 returns: Some(clt::I64),    module: None },
    // Receveur `T|null` d'un accès de membre (null → exception)
    BuiltinDesc { name: "__null_receiver",     params: &[clt::I64, clt::I64],                 returns: None,              module: None },

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
    BuiltinDesc { name: "__map_new",         params: &[],                                     returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__map_set",         params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: None },
    BuiltinDesc { name: "__map_get",         params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },

    // ── Libération ────────────────────────────────────────────────────────────
    // La libération des valeurs tas (string/array/map/objet) et la fermeture
    // des champs ressource passent par le comptage de références : voir
    // src/lower/builder.d/rc_layout.rs et runtime/src/rc.rs.

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

    // ── Arithmétique dynamique avec un opérande `mixed` ───────────────────────
    // Voir lower::expr::lower (Expr::Binary Add/Sub/Mul/Div/Mod) et la doc de
    // ces fonctions runtime pour le détail du dispatch.
    BuiltinDesc { name: "__dyn_add",              params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__dyn_sub",              params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__dyn_mul",              params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__dyn_div",              params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__mixed_to_int",         params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__mixed_to_float",       params: &[clt::I64],                        returns: Some(clt::F64),    module: None },

    // ── Gestion des erreurs (try/on/fail) — toujours disponibles ─────────────
    // `emit`/`message<T>` dans un `try` (Cas A, voir
    // docs/roadmap.d/langage-emit-iterable.md et
    // src/lower/builder.d/message_gen.rs) : `setjmp` est déclaré ICI pour
    // être appelable DIRECTEMENT depuis le code généré (pas via un wrapper
    // Rust — setjmp doit capturer la frame de la fonction __resume
    // elle-même, seule façon de le rejouer correctement à chaque reprise).
    // Résolu au lien contre la libc, exactement comme pour
    // `runtime/src/lib.rs` qui l'utilise déjà en interne.
    BuiltinDesc { name: "setjmp",                       params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_try_enter",             params: &[],                                returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_try_exit",              params: &[],                                returns: None,              module: None },
    BuiltinDesc { name: "__ocara_try_exec",             params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_try_exec_with_captures", params: &[clt::I64, clt::I64, clt::I64],    returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_try_set_return",       params: &[clt::I64],                        returns: None,              module: None },
    BuiltinDesc { name: "__ocara_try_take_return",      params: &[],                                returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_fail",                 params: &[clt::I64, clt::I64],              returns: None,              module: None },
    BuiltinDesc { name: "__ocara_type_matches",         params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__task_spawn",           params: &[clt::I64, clt::I64, clt::I64],    returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__task_resolve",         params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__unbox_float",          params: &[clt::I64],                        returns: Some(clt::F64),    module: None },
    BuiltinDesc { name: "__unbox_bool",           params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__unbox_int",             params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_unhandled_fail", params: &[clt::I64],                        returns: None,              module: None },
    
    // Allocation d'objet tas (toujours disponible)
    BuiltinDesc { name: "__free_obj",             params: &[clt::I64, clt::I64],              returns: None,               module: None },
    BuiltinDesc { name: "__alloc_obj",            params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__rc_retain",            params: &[clt::I64],                                 returns: None,               module: None },
    BuiltinDesc { name: "__rc_release",           params: &[clt::I64],                                 returns: None,               module: None },
    BuiltinDesc { name: "__rc_mark_raw",          params: &[clt::I64],                                 returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__rc_mark_raw_kind",     params: &[clt::I64, clt::I64],                       returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__rc_unwind_push",       params: &[clt::I64, clt::I64],                     returns: None,               module: None },
    BuiltinDesc { name: "__rc_unwind_pop",        params: &[],                                         returns: None,               module: None },
    BuiltinDesc { name: "__rc_resource_push",     params: &[clt::I64, clt::I64],                     returns: None,               module: None },
    BuiltinDesc { name: "__rc_resource_pop",      params: &[clt::I64],                                 returns: None,               module: None },
    BuiltinDesc { name: "__rc_collect_cycles",    params: &[],                                         returns: None,               module: None },
    BuiltinDesc { name: "__alloc_class_obj",      params: &[clt::I64, clt::I64, clt::I64],            returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__alloc_fat_ptr",        params: &[],                                returns: Some(clt::I64),    module: None },
    // Cellule verrouillée pour variables capturées (voir runtime/src/lib.rs).
    BuiltinDesc { name: "__alloc_gen",             params: &[clt::I64, clt::I64],                     returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__free_gen",              params: &[clt::I64, clt::I64],                     returns: None,               module: None },
    BuiltinDesc { name: "__alloc_env",             params: &[clt::I64, clt::I64],                     returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__alloc_locked_cell",     params: &[clt::I64],                              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__locked_cell_get",       params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__locked_cell_set",       params: &[clt::I64, clt::I64],              returns: None,              module: None },
    
    // Conversion string — sans heuristique pointeur
    BuiltinDesc { name: "__str_from_int",         params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_to_str",         params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__system_os",            params: &[],                                returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__system_arch",          params: &[],                                returns: Some(clt::I64),    module: None },
    // Profilage par fonction (`--profile`, voir src/ir/profile.rs)
    BuiltinDesc { name: "__prof_enter",           params: &[clt::I64],                        returns: None,              module: None },
    BuiltinDesc { name: "__prof_exit",            params: &[clt::I64],                        returns: None,              module: None },
    // Bilan des fuites (`--leak-check`, voir src/ir/leak_check.rs)
    BuiltinDesc { name: "__leak_check_init",      params: &[clt::I64],                        returns: None,              module: None },
    BuiltinDesc { name: "__leak_enter",           params: &[clt::I64],                        returns: None,              module: None },
    BuiltinDesc { name: "__leak_exit",            params: &[clt::I64],                        returns: None,              module: None },
    // Champ d'un binding `on e` générique (voir runtime/src/exception.rs)
    BuiltinDesc { name: "__exc_get_field",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    // Variable de générateur capturée par une closure (voir runtime/src/lib.rs)
    BuiltinDesc { name: "__gen_cell",             params: &[clt::I64, clt::I64, clt::I64],    returns: Some(clt::I64),    module: None },
    // Constantes globales calculées (voir runtime/src/global_const.rs)
    BuiltinDesc { name: "__global_const_slot",    params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    // Pile alternative + message de débordement de pile, appelé en tête du
    // `main` généré (voir runtime/src/stack_guard.rs)
    BuiltinDesc { name: "__ocara_runtime_init",   params: &[],                                returns: None,              module: None },
    // Couverture par ligne `--coverage` (voir runtime/src/coverage.rs)
    BuiltinDesc { name: "__cov_init",             params: &[clt::I64],                        returns: None,              module: None },
    BuiltinDesc { name: "__cov_point",            params: &[clt::I64, clt::I64, clt::I64],    returns: None,              module: None },
    BuiltinDesc { name: "__cov_hit",              params: &[clt::I64],                        returns: None,              module: None },

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
