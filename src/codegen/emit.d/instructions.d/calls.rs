/// Émission des instructions d'appels de fonction

use std::collections::HashMap;
use cranelift_codegen::ir::{types as clt, AbiParam, InstBuilder, MemFlags, Signature, Value as CrValue};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{FuncId, Module};
use cranelift_object::ObjectModule;
use crate::ir::inst::Inst;
use super::super::error::CgResult;

pub fn emit_calls(
    builder: &mut FunctionBuilder,
    inst: &Inst,
    vars: &[Variable],
    module: &mut ObjectModule,
    func_ids: &HashMap<String, FuncId>,
    param_types: &HashMap<String, Vec<cranelift_codegen::ir::Type>>,
    ret_types: &HashMap<String, cranelift_codegen::ir::Type>,
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
        Inst::Call { dest, func, args, .. } => {
            // Récupère les types attendus des paramètres (pour les bitcasts F64↔I64)
            let empty_params: Vec<cranelift_codegen::ir::Type> = vec![];
            let expected_params = param_types.get(func.as_str()).unwrap_or(&empty_params);

            let arg_vals: Vec<CrValue> = args.iter().enumerate().map(|(i, a)| {
                let v = use_var!(a);
                // Si le builtin attend F64 mais la variable est stockée en I64 (bitcast float),
                // on rebitcast I64 → F64 pour respecter la convention d'appel
                if expected_params.get(i).copied() == Some(clt::F64) {
                    builder.ins().bitcast(clt::F64, MemFlags::new(), v)
                } else {
                    v
                }
            }).collect();

            if let Some(&fid) = func_ids.get(func.as_str()) {
                let fref = module.declare_func_in_func(fid, builder.func);
                let call = builder.ins().call(fref, &arg_vals);
                if let Some(d) = dest {
                    let results = builder.inst_results(call);
                    if !results.is_empty() {
                        let result = results[0];
                        // Si le retour est F64, on le bitcast en I64 pour stockage uniforme
                        let final_val = if ret_types.get(func.as_str()).copied() == Some(clt::F64) {
                            builder.ins().bitcast(clt::I64, MemFlags::new(), result)
                        } else {
                            result
                        };
                        def!(d, final_val);
                    }
                }
            }
            // Si la fonction n'est pas connue, on ignore (runtime résolution)
        }

        Inst::CallIndirect { dest, callee, args, .. } => {
            let callee_val = use_var!(callee);
            let arg_vals: Vec<CrValue> = args.iter().map(|a| use_var!(a)).collect();
            // Signature générique I64* → I64
            let call_conv = builder.func.signature.call_conv;
            let mut sig = Signature::new(call_conv);
            for _ in &arg_vals {
                sig.params.push(AbiParam::new(clt::I64));
            }
            sig.returns.push(AbiParam::new(clt::I64));
            let sig_ref = builder.import_signature(sig);
            let call = builder.ins().call_indirect(sig_ref, callee_val, &arg_vals);
            if let Some(d) = dest {
                let results = builder.inst_results(call);
                if !results.is_empty() {
                    def!(d, results[0]);
                }
            }
        }

        Inst::FuncAddr { dest, func } => {
            if let Some(&fid) = func_ids.get(func.as_str()) {
                let fref = module.declare_func_in_func(fid, builder.func);
                let addr = builder.ins().func_addr(clt::I64, fref);
                def!(dest, addr);
            }
        }

        _ => return Ok(false), // Pas une instruction d'appel
    }

    Ok(true) // Instruction traitée
}
