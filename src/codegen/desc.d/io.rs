use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module IO
pub const IO_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "IO_write",         params: &[clt::I64],                 returns: None,           module: Some("IO") },
    BuiltinDesc { name: "IO_writeInt",      params: &[clt::I64],                 returns: None,           module: Some("IO") },
    BuiltinDesc { name: "IO_writeFloat",    params: &[clt::F64],                 returns: None,           module: Some("IO") },
    BuiltinDesc { name: "IO_writeBool",     params: &[clt::I64],                 returns: None,           module: Some("IO") },
    BuiltinDesc { name: "IO_writeln",       params: &[clt::I64],                 returns: None,           module: Some("IO") },
    BuiltinDesc { name: "IO_writelnInt",    params: &[clt::I64],                 returns: None,           module: Some("IO") },
    BuiltinDesc { name: "IO_writelnFloat",  params: &[clt::F64],                 returns: None,           module: Some("IO") },
    BuiltinDesc { name: "IO_writelnBool",   params: &[clt::I64],                 returns: None,           module: Some("IO") },
    BuiltinDesc { name: "IO_read",          params: &[],                         returns: Some(clt::I64), module: Some("IO") },
    BuiltinDesc { name: "IO_readln",        params: &[],                         returns: Some(clt::I64), module: Some("IO") },
    BuiltinDesc { name: "IO_readInt",       params: &[],                         returns: Some(clt::I64), module: Some("IO") },
    BuiltinDesc { name: "IO_readFloat",     params: &[],                         returns: Some(clt::F64), module: Some("IO") },
    BuiltinDesc { name: "IO_readBool",      params: &[],                         returns: Some(clt::I64), module: Some("IO") },
    BuiltinDesc { name: "IO_readArray",     params: &[clt::I64],                 returns: Some(clt::I64), module: Some("IO") },
    BuiltinDesc { name: "IO_readMap",       params: &[clt::I64, clt::I64],       returns: Some(clt::I64), module: Some("IO") },
];
