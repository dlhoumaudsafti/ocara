use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module Convert
pub const CONVERT_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Convert_strToInt",         params: &[clt::I64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_strToFloat",       params: &[clt::I64],                         returns: Some(clt::F64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_strToBool",        params: &[clt::I64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_strToArray",       params: &[clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_strToMap",         params: &[clt::I64, clt::I64, clt::I64],     returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_intToStr",         params: &[clt::I64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_intToFloat",       params: &[clt::I64],                         returns: Some(clt::F64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_intToBool",        params: &[clt::I64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_floatToStr",       params: &[clt::F64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_floatToInt",       params: &[clt::F64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_floatToBool",      params: &[clt::F64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_boolToStr",        params: &[clt::I64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_boolToInt",        params: &[clt::I64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_boolToFloat",      params: &[clt::I64],                         returns: Some(clt::F64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_arrayToStr",       params: &[clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_arrayToMap",       params: &[clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_mapToStr",         params: &[clt::I64, clt::I64, clt::I64],     returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_mapKeysToArray",   params: &[clt::I64],                         returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_mapValuesToArray", params: &[clt::I64],                         returns: Some(clt::I64), module: Some("Convert") },
];
