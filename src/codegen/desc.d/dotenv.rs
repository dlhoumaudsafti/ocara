use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module DotEnv
pub const DOTENV_BUILTINS: &[BuiltinDesc] = &[
    // DotEnv::load() → void (sans paramètre)
    BuiltinDesc { 
        name: "DotEnv_load_0", 
        params: &[],          // Aucun paramètre
        returns: None,        // → void
        module: Some("DotEnv") 
    },
    
    // DotEnv::load(env) → void (avec paramètre)
    BuiltinDesc { 
        name: "DotEnv_load", 
        params: &[clt::I64],  // env (string pointer, peut être 0)
        returns: None,        // → void
        module: Some("DotEnv") 
    },
    
    // DotEnv::get(key) → string|null
    BuiltinDesc { 
        name: "DotEnv_get", 
        params: &[clt::I64],      // key (string pointer)
        returns: Some(clt::I64),  // → string|null (pointeur ou 0)
        module: Some("DotEnv") 
    },
];
