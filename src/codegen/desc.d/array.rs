use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

// Note: module: None pour permettre les méthodes d'instance sans import
// L'import est vérifié uniquement pour les appels statiques Array::method() dans lower/expr.rs
pub const ARRAY_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Array_len",         params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_push",        params: &[clt::I64, clt::I64],                   returns: None,              module: None },
    BuiltinDesc { name: "Array_pop",         params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_first",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_last",        params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_contains",    params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_indexOf",    params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_reverse",     params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_slice",       params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_join",        params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_sort",        params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_get",         params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "Array_set",         params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: None },
];
