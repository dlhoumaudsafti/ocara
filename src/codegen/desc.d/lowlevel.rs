use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

pub const LOWLEVEL_BUILTINS: &[BuiltinDesc] = &[
    // ── Internes (toujours disponibles, jamais appelables directement) ────────
    BuiltinDesc { name: "__str_concat",      params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__val_to_str",      params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__str_from_float",  params: &[clt::F64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__str_from_bool",   params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__box_float",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
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
    
    // ── Strict comparison operators (===, !==, <==, >==) ──────────────────────
    BuiltinDesc { name: "__cmp_eq_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__cmp_ne_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__cmp_le_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__cmp_ge_strict",        params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    
    // ── Gestion des erreurs (try/on/fail) — toujours disponibles ─────────────
    BuiltinDesc { name: "__ocara_try_exec",       params: &[clt::I64, clt::I64],              returns: None,              module: None },
    BuiltinDesc { name: "__ocara_fail",           params: &[clt::I64, clt::I64],              returns: None,              module: None },
    BuiltinDesc { name: "__ocara_type_matches",   params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
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
];
