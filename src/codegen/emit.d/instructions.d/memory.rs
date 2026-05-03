/// Émission des instructions mémoire

use std::collections::HashMap;
use cranelift_codegen::ir::{types as clt, InstBuilder, MemFlags, StackSlotData, StackSlotKind};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{FuncId, Module};
use cranelift_object::ObjectModule;
use crate::ir::inst::Inst;
use super::super::error::CgResult;

pub fn emit_memory(
    builder: &mut FunctionBuilder,
    inst: &Inst,
    vars: &[Variable],
    module: &mut ObjectModule,
    func_ids: &HashMap<String, FuncId>,
    class_layouts: &HashMap<String, Vec<(String, cranelift_codegen::ir::Type)>>,
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
        Inst::Alloca { dest, .. } => {
            // Slot de pile
            let slot = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot, 8,
            ));
            let addr = builder.ins().stack_addr(clt::I64, slot, 0);
            def!(dest, addr);
        }

        Inst::Store { ptr, src } => {
            let p = use_var!(ptr);
            let s = use_var!(src);
            builder.ins().store(MemFlags::new(), s, p, 0);
        }

        Inst::Load { dest, ptr, .. } => {
            let p = use_var!(ptr);
            let v = builder.ins().load(clt::I64, MemFlags::new(), p, 0);
            def!(dest, v);
        }

        Inst::Alloc { dest, class } => {
            // Dispatch selon la nature de l'allocation :
            //   "__fat_ptr"     → __alloc_fat_ptr()      (TAG_FUNCTION, sans arg)
            //   "__env_*" / "__*" → __alloc_obj(size)    (interne, sans tag)
            //   classe utilisateur → __alloc_class_obj(size) (TAG_OBJECT)
            if class == "__fat_ptr" {
                // Fat pointer : {func_ptr, env_ptr} — taille fixe 16 octets
                let alloc_fid = func_ids.get("__alloc_fat_ptr")
                    .copied()
                    .expect("__alloc_fat_ptr non déclaré");
                let fref = module.declare_func_in_func(alloc_fid, builder.func);
                let call = builder.ins().call(fref, &[]);
                let ptr  = builder.inst_results(call)[0];
                def!(dest, ptr);
            } else if class.starts_with("__") {
                // Allocations internes (closure envs, etc.) — sans tag
                let n_fields = class_layouts.get(class.as_str()).map(|f| f.len()).unwrap_or(1);
                let size     = (n_fields as i64) * 8;
                let size_val = builder.ins().iconst(clt::I64, size);
                let alloc_fid = func_ids.get("__alloc_obj")
                    .copied()
                    .expect("__alloc_obj non déclaré");
                let fref = module.declare_func_in_func(alloc_fid, builder.func);
                let call = builder.ins().call(fref, &[size_val]);
                let ptr  = builder.inst_results(call)[0];
                def!(dest, ptr);
            } else {
                // Instance de classe utilisateur — TAG_OBJECT
                let n_fields = class_layouts.get(class.as_str()).map(|f| f.len()).unwrap_or(1);
                let size     = (n_fields as i64) * 8;
                let size_val = builder.ins().iconst(clt::I64, size);
                let alloc_fid = func_ids.get("__alloc_class_obj")
                    .copied()
                    .expect("__alloc_class_obj non déclaré");
                let fref = module.declare_func_in_func(alloc_fid, builder.func);
                let call = builder.ins().call(fref, &[size_val]);
                let ptr  = builder.inst_results(call)[0];
                def!(dest, ptr);
            }
        }

        _ => return Ok(false), // Pas une instruction mémoire
    }

    Ok(true) // Instruction traitée
}
