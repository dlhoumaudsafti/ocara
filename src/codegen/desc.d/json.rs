use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module JSON (module: None pour méthodes d'instance)
pub const JSON_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "JSON_encode",   params: &[clt::I64], returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "JSON_decode",   params: &[clt::I64], returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "JSON_pretty",   params: &[clt::I64], returns: Some(clt::I64), module: None },
    BuiltinDesc { name: "JSON_minimize", params: &[clt::I64], returns: Some(clt::I64), module: None },
];
