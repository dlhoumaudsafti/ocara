use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

// Note: module: None pour permettre les méthodes d'instance sans import
// L'import est vérifié uniquement pour les appels statiques Map::method() dans lower/expr.rs
pub const MAP_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Map_size",          params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Map_has",           params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Map_get",           params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Map_set",           params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: None },
    BuiltinDesc { name: "Map_remove",        params: &[clt::I64, clt::I64],                   returns: None,              module: None },
    BuiltinDesc { name: "Map_keys",          params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Map_values",        params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Map_merge",         params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Map_isEmpty",      params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
];
