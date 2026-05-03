/// Émission des instructions logiques

use cranelift_codegen::ir::{types as clt, InstBuilder};
use cranelift_frontend::{FunctionBuilder, Variable};
use crate::ir::inst::Inst;
use super::super::error::CgResult;

pub fn emit_logical(
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
        Inst::And { dest, lhs, rhs } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = builder.ins().band(l, r);
            def!(dest, v);
        }

        Inst::Or { dest, lhs, rhs } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = builder.ins().bor(l, r);
            def!(dest, v);
        }

        Inst::Not { dest, src } => {
            let s = use_var!(src);
            let one = builder.ins().iconst(clt::I64, 1);
            let v = builder.ins().bxor(s, one);
            def!(dest, v);
        }

        _ => return Ok(false), // Pas une instruction logique
    }

    Ok(true) // Instruction traitée
}
