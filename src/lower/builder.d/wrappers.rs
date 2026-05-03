/// Wrappers pour fonctions et async

use std::collections::HashMap;
use crate::ir::func::IrParam;
use crate::ir::inst::{Inst, Value};
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use super::types::LowerBuilder;

/// Génère `__fn_wrap_NAME(__env:ptr, params...) -> ret { return NAME(params) }`
/// Permet d'appeler n'importe quelle fonction via fat pointer {func_ptr, env_ptr}.
pub fn generate_wrapper(
    module:        &mut IrModule,
    original_name: &str,
    wrapper_name:  &str,
    param_tys:     &[IrType],
    ret_ty:        IrType,
    fn_ret_types:  &HashMap<String, IrType>,
) {
    let ir_func = {
        // Params : __env (ignoré) + params originaux
        let ir_params: Vec<IrParam> = {
            let mut p = vec![IrParam { name: "__env".into(), ty: IrType::Ptr, slot: Value(0) }];
            for (i, ty) in param_tys.iter().enumerate() {
                p.push(IrParam { name: format!("__p{}", i), ty: ty.clone(), slot: Value(0) });
            }
            p
        };

        // Convention uniforme : tous les wrappers retournent I64
        // (void → retourne 0, les callers ignorent le résultat)
        let wrapper_ret_ty = if ret_ty == IrType::Void { IrType::I64 } else { ret_ty.clone() };

        let mut builder = LowerBuilder::new(module, wrapper_name.into(), ir_params, wrapper_ret_ty.clone());
        builder.fn_ret_types = fn_ret_types.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        // Setup params (pattern alloca + receiver)
        let mut updated_params = Vec::new();
        let env_slot = builder.declare_local("__env", IrType::Ptr, false);
        let env_recv = builder.new_value();
        builder.emit(Inst::Store { ptr: env_slot, src: env_recv.clone() });
        updated_params.push(IrParam { name: "__env".into(), ty: IrType::Ptr, slot: env_recv });

        let mut call_args = Vec::new();
        for (i, ty) in param_tys.iter().enumerate() {
            let pname = format!("__p{}", i);
            let slot  = builder.declare_local(&pname, ty.clone(), false);
            let recv  = builder.new_value();
            builder.emit(Inst::Store { ptr: slot, src: recv.clone() });
            updated_params.push(IrParam { name: pname.clone(), ty: ty.clone(), slot: recv });
            let (val, _) = builder.load_local(&pname).unwrap();
            call_args.push(val);
        }
        builder.func.params = updated_params;

        // Appel direct à la fonction originale (convention sans env)
        if ret_ty != IrType::Void {
            let dest = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(dest.clone()),
                func:   original_name.into(),
                args:   call_args,
                ret_ty: ret_ty.clone(),
            });
            builder.emit(Inst::Return { value: Some(dest) });
        } else {
            builder.emit(Inst::Call {
                dest:   None,
                func:   original_name.into(),
                args:   call_args,
                ret_ty: IrType::Void,
            });
            let zero = builder.new_value();
            builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
            builder.emit(Inst::Return { value: Some(zero) });
        }

        builder.func
    };

    module.add_function(ir_func);
}

/// Génère un wrapper async `__async_wrap_FUNCNAME(env: i64) -> i64`
/// Lit les arguments depuis l'env heap (env[0], env[8], ...) et appelle la fonction réelle.
pub fn generate_async_wrapper(
    module:        &mut IrModule,
    original_name: &str,
    wrapper_name:  &str,
    param_tys:     &[IrType],
    ret_ty:        IrType,
    fn_ret_types:  &HashMap<String, IrType>,
) {
    let ir_func = {
        let ir_params = vec![IrParam { name: "__env".into(), ty: IrType::I64, slot: Value(0) }];

        let mut builder = LowerBuilder::new(module, wrapper_name.into(), ir_params, IrType::I64);
        builder.fn_ret_types = fn_ret_types.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        // Paramètre : slot alloca pour __env
        let env_slot = builder.declare_local("__env", IrType::I64, false);
        let env_recv = builder.new_value();
        builder.emit(Inst::Store { ptr: env_slot, src: env_recv.clone() });
        builder.func.params = vec![IrParam { name: "__env".into(), ty: IrType::I64, slot: env_recv }];

        // Charger __env
        let (env_val, _) = builder.load_local("__env").unwrap();

        // Lire chaque arg depuis env[i*8]
        let mut call_args = Vec::new();
        for (i, ty) in param_tys.iter().enumerate() {
            let dest = builder.new_value();
            builder.emit(Inst::GetField {
                dest:   dest.clone(),
                obj:    env_val.clone(),
                field:  format!("__arg{}", i),
                ty:     ty.clone(),
                offset: (i * 8) as i32,
            });
            call_args.push(dest);
        }

        // Appel direct
        if ret_ty != IrType::Void {
            let result = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(result.clone()),
                func:   original_name.into(),
                args:   call_args,
                ret_ty: ret_ty.clone(),
            });
            // Convertir en I64 si nécessaire
            let final_val = if ret_ty == IrType::I64 || ret_ty == IrType::Ptr {
                // I64 et Ptr (string, object, array, map, Function) sont déjà
                // des valeurs i64 valides — pas de boxing nécessaire.
                result
            } else {
                let boxed = builder.new_value();
                match ret_ty {
                    IrType::F64 => builder.emit(Inst::Call {
                        dest: Some(boxed.clone()), func: "__box_float".into(),
                        args: vec![result], ret_ty: IrType::Ptr,
                    }),
                    IrType::Bool => builder.emit(Inst::Call {
                        dest: Some(boxed.clone()), func: "__box_bool".into(),
                        args: vec![result], ret_ty: IrType::Ptr,
                    }),
                    _ => {
                        // Void ou autre : retourner 0
                        builder.emit(Inst::ConstInt { dest: boxed.clone(), value: 0 });
                    }
                }
                boxed
            };
            builder.emit(Inst::Return { value: Some(final_val) });
        } else {
            builder.emit(Inst::Call {
                dest:   None,
                func:   original_name.into(),
                args:   call_args,
                ret_ty: IrType::Void,
            });
            let zero = builder.new_value();
            builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
            builder.emit(Inst::Return { value: Some(zero) });
        }

        builder.func
    };

    module.add_function(ir_func);
}
