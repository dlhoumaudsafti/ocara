use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module Mutex
pub const MUTEX_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Mutex_init",    params: &[clt::I64], returns: None,           module: Some("Mutex") },
    BuiltinDesc { name: "Mutex_lock",    params: &[clt::I64], returns: None,           module: Some("Mutex") },
    BuiltinDesc { name: "Mutex_unlock",  params: &[clt::I64], returns: None,           module: Some("Mutex") },
    BuiltinDesc { name: "Mutex_tryLock", params: &[clt::I64], returns: Some(clt::I64), module: Some("Mutex") },
];
