use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module Regex
pub const REGEX_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Regex_test",       params: &[clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("Regex") },
    BuiltinDesc { name: "Regex_find",       params: &[clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("Regex") },
    BuiltinDesc { name: "Regex_findAll",    params: &[clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("Regex") },
    BuiltinDesc { name: "Regex_replace",    params: &[clt::I64, clt::I64, clt::I64],     returns: Some(clt::I64), module: Some("Regex") },
    BuiltinDesc { name: "Regex_replaceAll", params: &[clt::I64, clt::I64, clt::I64],     returns: Some(clt::I64), module: Some("Regex") },
    BuiltinDesc { name: "Regex_split",      params: &[clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("Regex") },
    BuiltinDesc { name: "Regex_count",      params: &[clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("Regex") },
    BuiltinDesc { name: "Regex_extract",    params: &[clt::I64, clt::I64, clt::I64],     returns: Some(clt::I64), module: Some("Regex") },
];
