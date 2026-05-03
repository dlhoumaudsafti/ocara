/// Lowering des fonctions anonymes (closures)

use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;
use crate::ir::func::IrParam;
use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

pub fn lower_nameless_fn(
    module:        &mut crate::ir::module::IrModule,
    anon_name:     &str,
    params:        &[Param],
    _ret_ty:        IrType,  // ignoré — toutes les closures retournent I64 (convention uniforme)
    body:          &Block,
    captures:      &[(String, IrType)],
    fn_ret_types:  &HashMap<String, IrType>,
    fn_param_types: &HashMap<String, Vec<IrType>>,
    current_class: &Option<String>,
    var_class:     &HashMap<String, String>,
    func_vars:     &HashSet<String>,
    has_defaults:  bool,
) {
    // Enregistrer le layout de l'env dans class_layouts
    if !captures.is_empty() || has_defaults {
        let env_class  = format!("__env_{}", anon_name);
        let mut env_fields: Vec<(String, IrType)> = captures.iter().enumerate()
            .map(|(i, (_, ty))| (format!("__cap_{}", i), ty.clone()))
            .collect();
        
        // Ajouter les champs pour les valeurs par défaut
        if has_defaults {
            for (i, param) in params.iter().enumerate() {
                if param.default_value.is_some() {
                    env_fields.push((format!("__default_{}", i), IrType::from_ast(&param.ty)));
                }
            }
        }
        
        module.class_layouts.insert(env_class, env_fields);
    }

    let ir_func = {
        let ir_params: Vec<IrParam> = {
            let mut p = vec![IrParam { name: "__env".into(), ty: IrType::Ptr, slot: Value(0) }];
            for param in params {
                p.push(IrParam { name: param.name.clone(), ty: IrType::from_ast(&param.ty), slot: Value(0) });
            }
            p
        };

        // Convention uniforme : toutes les closures retournent I64
        // (void → retourne 0, les callers ignorent le résultat)
        let mut builder = LowerBuilder::new(module, anon_name.into(), ir_params, IrType::I64);
        builder.fn_ret_types   = fn_ret_types.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        builder.fn_param_types = fn_param_types.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        builder.current_class  = current_class.clone();
        builder.var_class      = var_class.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        builder.func_vars      = func_vars.clone();

        // Setup params (alloca + receiver)
        let mut updated_params: Vec<IrParam> = Vec::new();
        let env_slot = builder.declare_local("__env", IrType::Ptr, false);
        let env_recv = builder.new_value();
        builder.emit(Inst::Store { ptr: env_slot, src: env_recv.clone() });
        updated_params.push(IrParam { name: "__env".into(), ty: IrType::Ptr, slot: env_recv });

        for param in params {
            let ir_ty = IrType::from_ast(&param.ty);
            if let Type::Map(_, _) = &param.ty  { builder.map_vars.insert(param.name.clone()); }
            if let Type::Function { ret_ty, .. }  = &param.ty  {
                builder.func_vars.insert(param.name.clone());
                builder.func_ret_types.insert(param.name.clone(), IrType::from_ast(ret_ty));
            }
            let slot = builder.declare_local(&param.name, ir_ty.clone(), false);
            let recv = builder.new_value();
            builder.emit(Inst::Store { ptr: slot, src: recv.clone() });
            updated_params.push(IrParam { name: param.name.clone(), ty: ir_ty, slot: recv });
        }
        builder.func.params = updated_params;

        // Si paramètres avec valeurs par défaut, vérifier et remplacer les sentinelles (0)
        if has_defaults {
            let env_val = builder.load_local("__env").map(|(v, _)| v).unwrap();
            for (i, param) in params.iter().enumerate() {
                if param.default_value.is_some() {
                    let param_val = builder.load_local(&param.name).map(|(v, _)| v).unwrap();
                    let ir_ty = IrType::from_ast(&param.ty);
                    
                    // Vérifier si le paramètre est 0 (sentinelle pour valeur non fournie)
                    let zero = builder.new_value();
                    match ir_ty {
                        IrType::I64 | IrType::Ptr => {
                            builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                        }
                        IrType::F64 => {
                            builder.emit(Inst::ConstFloat { dest: zero.clone(), value: 0.0 });
                        }
                        IrType::Bool => {
                            builder.emit(Inst::ConstBool { dest: zero.clone(), value: false });
                        }
                        _ => {
                            builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                        }
                    }
                    
                    let is_sentinel = builder.new_value();
                    builder.emit(Inst::CmpEq {
                        dest: is_sentinel.clone(),
                        lhs: param_val,
                        rhs: zero,
                        ty: ir_ty.clone(),
                    });
                    
                    // Si c'est une sentinelle, charger la valeur par défaut de l'env
                    let then_bb = builder.new_block();
                    let else_bb = builder.new_block();
                    let merge_bb = builder.new_block();
                    
                    builder.emit(Inst::Branch {
                        cond: is_sentinel,
                        then_bb: then_bb.clone(),
                        else_bb: else_bb.clone(),
                    });
                    
                    // Branch then : charger la valeur par défaut
                    builder.switch_to(&then_bb);
                    let default_offset = (captures.len() + i) * 8;
                    let default_val = builder.new_value();
                    builder.emit(Inst::GetField {
                        dest: default_val.clone(),
                        obj: env_val.clone(),
                        field: format!("__default_{}", i),
                        ty: ir_ty.clone(),
                        offset: default_offset as i32,
                    });
                    builder.emit(Inst::Store {
                        ptr: builder.slot_of_local(&param.name).unwrap(),
                        src: default_val,
                    });
                    builder.emit(Inst::Jump { target: merge_bb.clone() });
                    
                    // Branch else : garder la valeur reçue
                    builder.switch_to(&else_bb);
                    builder.emit(Inst::Jump { target: merge_bb.clone() });
                    
                    // Merge
                    builder.switch_to(&merge_bb);
                }
            }
        }

        // Enregistrer les captures comme accès directs à l'env struct (GetField/SetField).
        // Cela garantit que les mutations (ex: x = x + 1) sont persistantes d'un appel
        // à l'autre : les reads/writes vont directement dans le struct env sur le tas.
        if !captures.is_empty() {
            let env_val = builder.load_local("__env").map(|(v, _)| v).unwrap();
            for (i, (cap_name, cap_ty)) in captures.iter().enumerate() {
                builder.captured_vars.insert(
                    cap_name.clone(),
                    (env_val.clone(), i, cap_ty.clone()),
                );
                // Propager le type de classe si c'est une instance
                if let Some(cls) = var_class.get(cap_name.as_str()) {
                    builder.var_class.insert(cap_name.clone(), cls.clone());
                }
            }
        }

        crate::lower::stmt::lower_block(&mut builder, body);

        // Toujours retourner I64(0) en fallthrough (convention uniforme CallIndirect)
        if !builder.is_terminated() {
            let z = builder.new_value();
            builder.emit(Inst::ConstInt { dest: z.clone(), value: 0 });
            builder.emit(Inst::Return { value: Some(z) });
        }

        builder.func
    };

    module.add_function(ir_func);
}
