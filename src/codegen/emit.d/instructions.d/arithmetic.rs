/// Émission des instructions arithmétiques

use cranelift_codegen::ir::{types as clt, InstBuilder, MemFlags};
use cranelift_frontend::{FunctionBuilder, Variable};
use crate::ir::inst::Inst;
use crate::ir::types::IrType;
use super::super::error::CgResult;

pub fn emit_arithmetic(
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

        Inst::Mod { dest, lhs, rhs, .. } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = builder.ins().srem(l, r);
            def!(dest, v);
        }

        Inst::Neg { dest, src, .. } => {
            let s = use_var!(src);
            let v = builder.ins().ineg(s);
            def!(dest, v);
        }

        _ => return Ok(false), // Pas une instruction arithmétique
    }

    Ok(true) // Instruction traitée
}
