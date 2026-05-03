use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module System
pub const SYSTEM_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "System_exec",        params: &[clt::I64],               returns: Some(clt::I64), module: Some("System") },
    BuiltinDesc { name: "System_passthrough", params: &[clt::I64],               returns: Some(clt::I64), module: Some("System") },
    BuiltinDesc { name: "System_execCode",    params: &[clt::I64],               returns: Some(clt::I64), module: Some("System") },
    BuiltinDesc { name: "System_exit",        params: &[clt::I64],               returns: None,           module: Some("System") },
    BuiltinDesc { name: "System_env",         params: &[clt::I64],               returns: Some(clt::I64), module: Some("System") },
    BuiltinDesc { name: "System_setEnv",      params: &[clt::I64, clt::I64],     returns: None,           module: Some("System") },
    BuiltinDesc { name: "System_cwd",         params: &[],                       returns: Some(clt::I64), module: Some("System") },
    BuiltinDesc { name: "System_sleep",       params: &[clt::I64],               returns: None,           module: Some("System") },
    BuiltinDesc { name: "System_pid",         params: &[],                       returns: Some(clt::I64), module: Some("System") },
    BuiltinDesc { name: "System_args",        params: &[],                       returns: Some(clt::I64), module: Some("System") },
];
