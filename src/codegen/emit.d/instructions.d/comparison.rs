/// Émission des instructions de comparaison

use cranelift_codegen::ir::{types as clt, InstBuilder, MemFlags, condcodes::{IntCC, FloatCC}};
use cranelift_frontend::{FunctionBuilder, Variable};
use crate::ir::inst::Inst;
use crate::ir::types::IrType;
use super::super::error::CgResult;

pub fn emit_comparison(
    builder: &mut FunctionBuilder,
    inst: &Inst,
    vars: &[Variable],
) -> CgResult<bool> {
    macro_rules! def {
        ($v:expr, $val:expr) => {
            builder.def_var(vars[$v.0 as usize], $val)
        };
    }
    macro_rules! use_var {
        ($v:expr) => {
            builder.use_var(vars[$v.0 as usize])
        };
    }

    match inst {
        Inst::CmpEq { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(FloatCC::Equal, lf, rf)
            } else { builder.ins().icmp(IntCC::Equal, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }

        Inst::CmpNe { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(FloatCC::NotEqual, lf, rf)
            } else { builder.ins().icmp(IntCC::NotEqual, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }

        Inst::CmpLt { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(FloatCC::LessThan, lf, rf)
            } else { builder.ins().icmp(IntCC::SignedLessThan, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }

        Inst::CmpLe { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(FloatCC::LessThanOrEqual, lf, rf)
            } else { builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }

        Inst::CmpGt { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(FloatCC::GreaterThan, lf, rf)
            } else { builder.ins().icmp(IntCC::SignedGreaterThan, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }

        Inst::CmpGe { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(FloatCC::GreaterThanOrEqual, lf, rf)
            } else { builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }

        _ => return Ok(false), // Pas une instruction de comparaison
    }

    Ok(true) // Instruction traitée
}
