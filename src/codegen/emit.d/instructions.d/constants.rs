/// Émission des instructions de constantes

use cranelift_codegen::ir::{types as clt, InstBuilder, MemFlags};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{Linkage, Module};
use cranelift_object::ObjectModule;
use crate::ir::inst::Inst;
use super::super::error::{CodegenError, CgResult};

pub fn emit_constants(
    builder: &mut FunctionBuilder,
    inst: &Inst,
    vars: &[Variable],
    module: &mut ObjectModule,
) -> CgResult<bool> {
    macro_rules! def {
        ($v:expr, $val:expr) => {
            builder.def_var(vars[$v.0 as usize], $val)
        };
    }

    match inst {
        Inst::Nop => {}

        Inst::ConstInt { dest, value } => {
            let v = builder.ins().iconst(clt::I64, *value);
            def!(dest, v);
        }

        Inst::ConstFloat { dest, value } => {
            let v = builder.ins().f64const(*value);
            // store as bitcast i64 for uniform variable representation
            let v64 = builder.ins().bitcast(clt::I64, MemFlags::new(), v);
            def!(dest, v64);
        }

        Inst::ConstBool { dest, value } => {
            let v = builder.ins().iconst(clt::I64, *value as i64);
            def!(dest, v);
        }

        Inst::ConstStr { dest, idx } => {
            let ptr = string_address(builder, module, *idx)?;
            def!(dest, ptr);
        }

        _ => return Ok(false), // Pas une instruction de constante
    }

    Ok(true) // Instruction traitée
}

/// Adresse d'une chaîne littérale `__str_N`. En-tête de 16 octets (longueur
/// puis `TAG_STRING`) : les données, et donc la valeur, sont à `+16` — le
/// tag reste lisible à `val - 8`.
pub fn string_address(builder: &mut FunctionBuilder, module: &mut ObjectModule, idx: u32) -> CgResult<cranelift_codegen::ir::Value> {
    let name = format!("__str_{}", idx);
    let data_id = module
        .declare_data(&name, Linkage::Local, false, false)
        .map_err(|e| CodegenError(format!("declare_data({}): {}", name, e)))?;
    let gv  = module.declare_data_in_func(data_id, builder.func);
    let raw = builder.ins().global_value(clt::I64, gv);
    let sixteen = builder.ins().iconst(clt::I64, 16);
    Ok(builder.ins().iadd(raw, sixteen))
}
