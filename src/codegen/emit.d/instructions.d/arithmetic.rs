/// Émission des instructions arithmétiques

use cranelift_codegen::ir::{types as clt, InstBuilder, MemFlags};
use cranelift_frontend::FunctionBuilder;
use super::super::value_slots::ValueSlots;
use crate::ir::inst::Inst;
use crate::ir::types::IrType;
use super::super::error::CgResult;

pub fn emit_arithmetic(
    builder: &mut FunctionBuilder,
    inst: &Inst,
    vars: &ValueSlots,
) -> CgResult<bool> {
    macro_rules! def {
        ($v:expr, $val:expr) => {
            vars.def(builder, $v.0, $val)
        };
    }
    macro_rules! use_var {
        ($v:expr) => {
            vars.get(builder, $v.0)
        };
    }

    match inst {
        Inst::Add { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                let res = builder.ins().fadd(lf, rf);
                builder.ins().bitcast(clt::I64, MemFlags::new(), res)
            } else { builder.ins().iadd(l, r) };
            def!(dest, v);
        }

        Inst::Sub { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                let res = builder.ins().fsub(lf, rf);
                builder.ins().bitcast(clt::I64, MemFlags::new(), res)
            } else { builder.ins().isub(l, r) };
            def!(dest, v);
        }

        Inst::Mul { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                let res = builder.ins().fmul(lf, rf);
                builder.ins().bitcast(clt::I64, MemFlags::new(), res)
            } else { builder.ins().imul(l, r) };
            def!(dest, v);
        }

        Inst::Div { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                let res = builder.ins().fdiv(lf, rf);
                builder.ins().bitcast(clt::I64, MemFlags::new(), res)
            } else { builder.ins().sdiv(l, r) };
            def!(dest, v);
        }

        Inst::Mod { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            // Flottants : `x − trunc(x / y) × y` (signe du dividende, comme
            // `fmod`) — `srem` sur les bits donnait une valeur absurde.
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                let quot = builder.ins().fdiv(lf, rf);
                let whole = builder.ins().trunc(quot);
                let prod = builder.ins().fmul(whole, rf);
                let res = builder.ins().fsub(lf, prod);
                builder.ins().bitcast(clt::I64, MemFlags::new(), res)
            } else { builder.ins().srem(l, r) };
            def!(dest, v);
        }

        Inst::Neg { dest, src, ty } => {
            let s = use_var!(src);
            let v = if *ty == IrType::F64 {
                let sf = builder.ins().bitcast(clt::F64, MemFlags::new(), s);
                let neg = builder.ins().fneg(sf);
                builder.ins().bitcast(clt::I64, MemFlags::new(), neg)
            } else {
                builder.ins().ineg(s)
            };
            def!(dest, v);
        }

        _ => return Ok(false), // Pas une instruction arithmétique
    }

    Ok(true) // Instruction traitée
}
