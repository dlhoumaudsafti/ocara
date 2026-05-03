use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module Thread
pub const THREAD_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Thread_init",       params: &[clt::I64],           returns: None,           module: Some("Thread") },
    BuiltinDesc { name: "Thread_run",        params: &[clt::I64, clt::I64], returns: None,           module: Some("Thread") },
    BuiltinDesc { name: "Thread_join",       params: &[clt::I64],           returns: None,           module: Some("Thread") },
    BuiltinDesc { name: "Thread_detach",     params: &[clt::I64],           returns: None,           module: Some("Thread") },
    BuiltinDesc { name: "Thread_id",         params: &[clt::I64],           returns: Some(clt::I64), module: Some("Thread") },
    BuiltinDesc { name: "Thread_sleep",      params: &[clt::I64],           returns: None,           module: Some("Thread") },
    BuiltinDesc { name: "Thread_currentId",  params: &[],                   returns: Some(clt::I64), module: Some("Thread") },
];
