/// Émission des instructions mémoire

use std::collections::HashMap;
use cranelift_codegen::ir::{types as clt, InstBuilder, MemFlags, StackSlotData, StackSlotKind};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{FuncId, Module};
use cranelift_object::ObjectModule;
use crate::ir::inst::Inst;
use super::super::error::CgResult;
use super::constants::string_address;

pub fn emit_memory(
    builder: &mut FunctionBuilder,
    inst: &Inst,
    vars: &[Variable],
    module: &mut ObjectModule,
    func_ids: &HashMap<String, FuncId>,
    class_layouts: &HashMap<String, Vec<(String, cranelift_codegen::ir::Type)>>,
    class_ids: &HashMap<String, (i64, Option<u32>)>,
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

        Inst::AllocaWords { dest, words } => {
            let slot = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot, (*words).max(1) * 8,
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
            } else if class.starts_with("__env_") {
                // Env de closure : bloc compté dont les champs `__cap_*`
                // (en tête) sont des cellules — voir `__alloc_env`.
                let fields = class_layouts.get(class.as_str()).map(|f| f.as_slice()).unwrap_or(&[]);
                let n_caps = fields.iter().filter(|(name, _)| name.starts_with("__cap_")).count() as i64;
                let caps_val = builder.ins().iconst(clt::I64, n_caps);
                let fields_val = builder.ins().iconst(clt::I64, fields.len() as i64);
                let alloc_fid = func_ids.get("__alloc_env")
                    .copied()
                    .expect("__alloc_env non déclaré");
                let fref = module.declare_func_in_func(alloc_fid, builder.func);
                let call = builder.ins().call(fref, &[caps_val, fields_val]);
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
                // Instance de classe utilisateur — TAG_OBJECT + class_id
                // (identité réelle à l'exécution, voir IrModule::class_ids
                // et `__alloc_class_obj` dans runtime/src/lib.rs — nécessaire
                // pour `is ClassName`/`is InterfaceName` et le dispatch
                // dynamique, voir docs/roadmap.d/langage-interfaces.md).
                let n_fields = class_layouts.get(class.as_str()).map(|f| f.len()).unwrap_or(1);
                let size     = (n_fields as i64) * 8;
                let size_val = builder.ins().iconst(clt::I64, size);
                let (class_id, mask) = class_ids.get(class.as_str()).copied().unwrap_or((0, None));
                let class_id_val = builder.ins().iconst(clt::I64, class_id);
                let desc_val = match mask {
                    Some(idx) => string_address(builder, module, idx)?,
                    None => builder.ins().iconst(clt::I64, 0),
                };
                let alloc_fid = func_ids.get("__alloc_class_obj")
                    .copied()
                    .expect("__alloc_class_obj non déclaré");
                let fref = module.declare_func_in_func(alloc_fid, builder.func);
                let call = builder.ins().call(fref, &[size_val, class_id_val, desc_val]);
                let ptr  = builder.inst_results(call)[0];
                def!(dest, ptr);
            }
        }

        _ => return Ok(false), // Pas une instruction mémoire
    }

    Ok(true) // Instruction traitée
}
