use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module Math
pub const MATH_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Math_abs",    params: &[clt::I64],                   returns: Some(clt::I64), module: Some("Math") },
    BuiltinDesc { name: "Math_min",    params: &[clt::I64, clt::I64],         returns: Some(clt::I64), module: Some("Math") },
    BuiltinDesc { name: "Math_max",    params: &[clt::I64, clt::I64],         returns: Some(clt::I64), module: Some("Math") },
    BuiltinDesc { name: "Math_pow",    params: &[clt::I64, clt::I64],         returns: Some(clt::I64), module: Some("Math") },
    BuiltinDesc { name: "Math_clamp",  params: &[clt::I64, clt::I64, clt::I64], returns: Some(clt::I64), module: Some("Math") },
    BuiltinDesc { name: "Math_sqrt",   params: &[clt::F64],                   returns: Some(clt::F64), module: Some("Math") },
    BuiltinDesc { name: "Math_floor",  params: &[clt::F64],                   returns: Some(clt::I64), module: Some("Math") },
    BuiltinDesc { name: "Math_ceil",   params: &[clt::F64],                   returns: Some(clt::I64), module: Some("Math") },
    BuiltinDesc { name: "Math_round",  params: &[clt::F64],                   returns: Some(clt::I64), module: Some("Math") },
];
