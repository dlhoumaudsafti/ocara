use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

// Note: module: None pour permettre les méthodes d'instance sans import
// L'import est vérifié uniquement pour les appels statiques String::method() dans lower/expr.rs
pub const STRING_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "String_len",        params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "String_upper",      params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "String_lower",      params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "String_capitalize", params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "String_trim",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "String_replace",    params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "String_split",      params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "String_explode",    params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "String_between",    params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "String_empty",      params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
];
