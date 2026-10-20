/// Émission des instructions d'objets

use cranelift_codegen::ir::{types as clt, InstBuilder, MemFlags};
use cranelift_frontend::FunctionBuilder;
use super::super::value_slots::ValueSlots;
use crate::ir::inst::Inst;
use super::super::error::CgResult;

pub fn emit_objects(
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
        Inst::SetField { obj, field: _, src, offset } => {
            let o = use_var!(obj);
            let s = use_var!(src);
            builder.ins().store(MemFlags::new(), s, o, *offset);
        }

        Inst::GetField { dest, obj, field: _, ty: _, offset } => {
            let o = use_var!(obj);
            let v = builder.ins().load(clt::I64, MemFlags::new(), o, *offset);
            def!(dest, v);
        }

        Inst::Phi { dest, sources, .. } => {
            // Phi simplifié — utilise la première source disponible
            if let Some((val, _)) = sources.first() {
                let v = use_var!(val);
                def!(dest, v);
            }
        }

        _ => return Ok(false), // Pas une instruction d'objets
    }

    Ok(true) // Instruction traitée
}
