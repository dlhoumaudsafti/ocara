/// Dispatcher et organisation des instructions

mod constants;
mod arithmetic;
mod comparison;
mod logical;
mod memory;
mod control_flow;
mod calls;
mod objects;

use std::collections::HashMap;
use cranelift_codegen::ir::Block as CrBlock;
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::FuncId;
use cranelift_object::ObjectModule;
use crate::ir::inst::{BlockId, Inst};
use crate::ir::types::IrType;
use super::error::CgResult;

/// Émission d'une instruction Ocara IR → Cranelift IR
#[allow(clippy::too_many_arguments)]
pub fn emit_inst(
    builder:   &mut FunctionBuilder,
    inst:      &Inst,
    vars:      &[Variable],
    cl_blocks:   &HashMap<BlockId, CrBlock>,
    module:      &mut ObjectModule,
    func_ids:    &HashMap<String, FuncId>,
    param_types: &HashMap<String, Vec<cranelift_codegen::ir::Type>>,
    ret_types:   &HashMap<String, cranelift_codegen::ir::Type>,
    class_layouts: &HashMap<String, Vec<(String, cranelift_codegen::ir::Type)>>,
    func_ret_ty: &IrType,
) -> CgResult<()> {
    // Essayer chaque catégorie d'instructions dans l'ordre
    
    if constants::emit_constants(builder, inst, vars, module)? {
        return Ok(());
    }
    
    if arithmetic::emit_arithmetic(builder, inst, vars)? {
        return Ok(());
    }
    
    if comparison::emit_comparison(builder, inst, vars)? {
        return Ok(());
    }
    
    if logical::emit_logical(builder, inst, vars)? {
        return Ok(());
    }
    
    if memory::emit_memory(builder, inst, vars, module, func_ids, class_layouts)? {
        return Ok(());
    }
    
    if control_flow::emit_control_flow(builder, inst, vars, cl_blocks, func_ret_ty)? {
        return Ok(());
    }
    
    if calls::emit_calls(builder, inst, vars, module, func_ids, param_types, ret_types)? {
        return Ok(());
    }
    
    if objects::emit_objects(builder, inst, vars)? {
        return Ok(());
    }
    
    // Instruction non gérée — on ignore silencieusement
    Ok(())
}
