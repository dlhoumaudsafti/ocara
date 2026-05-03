/// Émission des instructions de contrôle de flux

use std::collections::HashMap;
use cranelift_codegen::ir::{types as clt, InstBuilder, MemFlags, condcodes::IntCC, Block as CrBlock};
use cranelift_frontend::{FunctionBuilder, Variable};
use crate::ir::inst::{BlockId, Inst};
use crate::ir::types::IrType;
use super::super::error::CgResult;

pub fn emit_control_flow(
    builder: &mut FunctionBuilder,
    inst: &Inst,
    vars: &[Variable],
    cl_blocks: &HashMap<BlockId, CrBlock>,
    func_ret_ty: &IrType,
) -> CgResult<bool> {
    macro_rules! use_var {
        ($v:expr) => {
            builder.use_var(vars[$v.0 as usize])
        };
    }

    match inst {
        Inst::Jump { target } => {
            let cl_bb = cl_blocks[target];
            builder.ins().jump(cl_bb, &[]);
        }

        Inst::Branch { cond, then_bb, else_bb } => {
            let c = use_var!(cond);
            let c1 = builder.ins().icmp_imm(IntCC::NotEqual, c, 0);
            let then_cl = cl_blocks[then_bb];
            let else_cl = cl_blocks[else_bb];
            builder.ins().brif(c1, then_cl, &[], else_cl, &[]);
        }

        Inst::Return { value } => {
            if let Some(v) = value {
                let rv = use_var!(v);
                // Si la fonction retourne F64, bitcaster i64 → f64 (les floats sont stockés bitcastés)
                let final_rv = if *func_ret_ty == IrType::F64 {
                    builder.ins().bitcast(clt::F64, MemFlags::new(), rv)
                } else {
                    rv
                };
                builder.ins().return_(&[final_rv]);
            } else {
                builder.ins().return_(&[]);
            }
        }

        _ => return Ok(false), // Pas une instruction de contrôle de flux
    }

    Ok(true) // Instruction traitée
}
