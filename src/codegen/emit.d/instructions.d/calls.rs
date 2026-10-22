/// Émission des instructions d'appels de fonction

use std::collections::HashMap;
use cranelift_codegen::ir::{types as clt, AbiParam, InstBuilder, MemFlags, Signature, Value as CrValue};
use cranelift_frontend::FunctionBuilder;
use super::super::value_slots::ValueSlots;
use cranelift_module::{FuncId, Module};
use cranelift_object::ObjectModule;
use crate::ir::inst::Inst;
use super::super::error::{CodegenError, CgResult};

pub fn emit_calls(
    builder: &mut FunctionBuilder,
    inst: &Inst,
    vars: &ValueSlots,
    module: &mut ObjectModule,
    func_ids: &HashMap<String, FuncId>,
    param_types: &HashMap<String, Vec<cranelift_codegen::ir::Type>>,
    ret_types: &HashMap<String, cranelift_codegen::ir::Type>,
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
                if matches!(func.as_str(), "__rc_retain" | "__rc_release") {
                    emit_counted_only(builder, arg_vals[0], |builder| { builder.ins().call(fref, &arg_vals); });
                    return Ok(true);
                }
                if let ("__box_int_for_mixed", Some(d)) = (func.as_str(), dest) {
                    let boxed = emit_box_int(builder, arg_vals[0], |builder| {
                        let call = builder.ins().call(fref, &arg_vals);
                        builder.inst_results(call)[0]
                    });
                    def!(d, boxed);
                    return Ok(true);
                }
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
            } else if func.starts_with("__") {
                // Fonction interne du runtime sans descripteur : bug du
                // compilateur, jamais à ignorer (l'appel disparaîtrait).
                return Err(CodegenError(format!("fonction interne non déclarée : {}", func)));
            } else if func.ends_with("_init") {
                // `use C()` sur une classe sans constructeur (ni hérité) :
                // rien à initialiser.
            } else {
                // Appel vers un symbole inexistant : la sema aurait dû le
                // refuser — l'ignorer rendait `null` en silence (voir
                // docs/roadmap.d/sema-appel-symbole-inconnu-accepte.md).
                return Err(CodegenError(format!("appel vers une fonction inconnue : {}", func)));
            }
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

/// Appelle `call` seulement si `val` peut être une valeur comptée : ni un
/// entier brut ou null (< 0x10000), ni un `int` de `mixed` rangé en ligne
/// (`is_inline_int`, runtime/src/lib.rs) — sur une boucle `mixed`, l'appel
/// qui ne faisait rien coûtait autant que le calcul.
fn emit_counted_only(builder: &mut FunctionBuilder, val: CrValue, call: impl FnOnce(&mut FunctionBuilder)) {
    use cranelift_codegen::ir::condcodes::IntCC;
    let counted = builder.create_block();
    let next = builder.create_block();
    let maybe_ptr = builder.ins().icmp_imm(IntCC::SignedGreaterThanOrEqual, val, 0x10000);
    let tag = builder.ins().band_imm(val, 0xC000_0000_0000_0007u64 as i64);
    let not_inline = builder.ins().icmp_imm(IntCC::NotEqual, tag, 0x4000_0000_0000_0004);
    let cond = builder.ins().band(maybe_ptr, not_inline);
    builder.ins().brif(cond, counted, &[], next, &[]);
    builder.switch_to_block(counted);
    call(builder);
    builder.ins().jump(next, &[]);
    builder.switch_to_block(next);
}

/// `__box_int_for_mixed` sans appel dans les cas courants, même encodage que
/// `box_int_if_needed` (runtime/src/lib.rs) : entier non nul sous 0x10000
/// (négatifs compris) brut, entier de 0 à 2⁵⁹ en ligne
/// (`0x4000_0000_0000_0004 | n << 3`) ; seul un plus grand appelle `call`.
fn emit_box_int(builder: &mut FunctionBuilder, n: CrValue, call: impl FnOnce(&mut FunctionBuilder) -> CrValue) -> CrValue {
    use cranelift_codegen::ir::condcodes::IntCC;
    let nonzero = builder.ins().icmp_imm(IntCC::NotEqual, n, 0);
    let small = builder.ins().icmp_imm(IntCC::SignedLessThan, n, 0x10000);
    let raw = builder.ins().band(nonzero, small);
    let shifted = builder.ins().ishl_imm(n, 3);
    let inline = builder.ins().bor_imm(shifted, 0x4000_0000_0000_0004);
    let fast = builder.ins().select(raw, n, inline);
    let too_big = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, n, 1 << 59);
    let not_raw = builder.ins().bxor_imm(raw, 1);
    let needs_cell = builder.ins().band(not_raw, too_big);
    let cell = builder.create_block();
    let next = builder.create_block();
    let result = builder.append_block_param(next, clt::I64);
    builder.ins().brif(needs_cell, cell, &[], next, &[fast.into()]);
    builder.switch_to_block(cell);
    let boxed = call(builder);
    builder.ins().jump(next, &[boxed.into()]);
    builder.switch_to_block(next);
    result
}
