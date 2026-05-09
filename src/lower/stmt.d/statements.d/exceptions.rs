/// Lowering des exceptions (try/raise)

use crate::parsing::ast::{Block, Expr, OnClause};
use crate::ir::func::IrParam;
use crate::ir::inst::Inst;
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::lower_expr;
use super::super::super::block::lower_block;

/// Lowering de `raise expr`
pub fn lower_raise(builder: &mut LowerBuilder, value: &Expr) {
    // Valeur de l'erreur
    let val = lower_expr(builder, value);

    // Type name : si l'expression est `use ClassName(...)`, on sait statiquement
    // quel type est levé → on peut filtrer avec `on e is ClassName`.
    let type_name_val = match value {
        Expr::New { class, .. } => {
            let idx = builder.module.intern_string(class);
            let dest = builder.new_value();
            builder.emit(Inst::ConstStr { dest: dest.clone(), idx });
            dest
        }
        _ => {
            // Pas de type statique connu (string, mixed, variable…)
            let dest = builder.new_value();
            builder.emit(Inst::ConstInt { dest: dest.clone(), value: 0 });
            dest
        }
    };

    builder.emit(Inst::Call {
        dest:   None,
        func:   "__ocara_fail".into(),
        args:   vec![val, type_name_val],
        ret_ty: IrType::Void,
    });

    // Le code après __ocara_fail est mort, mais l'IR doit être terminé.
    let dummy_ret = if builder.func.ret_ty != IrType::Void {
        let z = builder.new_value();
        builder.emit(Inst::ConstInt { dest: z.clone(), value: 0 });
        Some(z)
    } else {
        None
    };
    builder.emit(Inst::Return { value: dummy_ret });
}

/// Lowering de `try { body } on e [is Foo] { handler } …`
///
/// Modèle callback :
///   1. Le corps try      → fonction __try_body_N  () -> void
///   2. Le gestionnaire   → fonction __try_handler_N (err_val:i64, err_type:i64) -> void
///   3. Dans la fonction englobante :
///        body_addr    = FuncAddr(__try_body_N)
///        handler_addr = FuncAddr(__try_handler_N)
///        call __ocara_try_exec(body_addr, handler_addr)
///
/// __ocara_try_exec (C) fait le setjmp, appelle le corps, et en cas d'erreur
/// appelle le gestionnaire.  La frame de __ocara_try_exec reste vivante pendant
/// tout l'exécution du corps, ce qui garantit la validité du jmp_buf.
pub fn lower_try(builder: &mut LowerBuilder, body: &Block, handlers: &[OnClause]) {
    use crate::lower::expr::captures::collect_captures;
    use crate::ir::inst::Value;
    use std::collections::HashSet;
    
    // ID unique fondé sur le nombre de fonctions déjà dans le module
    let try_id = builder.module.functions.len();
    let body_fn_name    = format!("__try_body_{}", try_id);
    let handler_fn_name = format!("__try_handler_{}", try_id);

    // Collecter les captures du body (variables du scope parent référencées)
    let captures = collect_captures(body, &HashSet::new(), &builder.locals);

    // ── 1. Corps try ─────────────────────────────────────────────────────────
    // Le body a un type de retour Void : il ne retourne jamais normalement,
    // soit il bloque indéfiniment, soit il lève une exception
    let body_ret_ty = IrType::Void;
    let body_fn = {
        // Si captures, le body reçoit un Ptr vers le tableau
        let ir_params = if captures.is_empty() {
            vec![]
        } else {
            vec![IrParam {
                name: "__captures_ptr".into(),
                ty: IrType::Ptr,
                slot: Value(0),
            }]
        };
        
        let mut bb = LowerBuilder::new(
            &mut *builder.module,
            body_fn_name.clone(),
            ir_params,
            body_ret_ty.clone(),
        );
        bb.fn_ret_types  = builder.fn_ret_types.clone();
        bb.var_class     = builder.var_class.clone();
        bb.elem_types    = builder.elem_types.clone();
        bb.map_vars      = builder.map_vars.clone();
        bb.current_class = builder.current_class.clone();
        bb.parent_class  = builder.parent_class.clone();
        
        // Si captures, charger depuis le tableau pointé
        if !captures.is_empty() {
            // Créer une nouvelle Value pour le paramètre (receiver)
            let captures_ptr = bb.new_value();
            // Mettre à jour le slot du paramètre pour pointer vers cette Value
            bb.func.params[0].slot = captures_ptr.clone();
            
            // Charger chaque capture : *(captures_ptr + idx*8)
            for (idx, (name, ty)) in captures.iter().enumerate() {
                let offset_bytes = (idx * 8) as i64;
                
                // Calculer l'adresse de l'élément : captures_ptr + offset
                let offset_val = bb.new_value();
                bb.emit(Inst::ConstInt { dest: offset_val.clone(), value: offset_bytes });
                
                let elem_addr = bb.new_value();
                bb.emit(Inst::Add {
                    dest: elem_addr.clone(),
                    lhs: captures_ptr.clone(),
                    rhs: offset_val,
                    ty: IrType::I64,
                });
                
                // Charger la valeur depuis cette adresse
                let val = bb.new_value();
                bb.emit(Inst::Load { dest: val.clone(), ptr: elem_addr, ty: ty.clone() });
                
                // Créer le local et stocker
                let alloca_slot = bb.declare_local(name, ty.clone(), false);
                bb.emit(Inst::Store { ptr: alloca_slot, src: val });
                
                // Si c'est self, mettre à jour var_class pour indiquer la classe
                if name == "self" {
                    if let Some(cls) = &bb.current_class {
                        bb.var_class.insert("self".to_string(), cls.clone());
                    }
                }
            }
        }

        lower_block(&mut bb, body);
        
        if !bb.is_terminated() {
            // Return implicite : valeur dummy si type non-void
            let ret_val = if body_ret_ty != IrType::Void {
                let dummy = bb.new_value();
                bb.emit(Inst::ConstInt { dest: dummy.clone(), value: 0 });
                Some(dummy)
            } else {
                None
            };
            bb.emit(Inst::Return { value: ret_val });
        }
        bb.func   // move func out, drops bb, releases module reborrow
    };
    builder.module.add_function(body_fn);

    // ── 2. Gestionnaire ──────────────────────────────────────────────────────
    // Le handler doit avoir le même type de retour que la fonction englobante
    // car il peut contenir des return qui remontent à la fonction parente
    let handler_ret_ty = builder.func.ret_ty.clone();
    let handler_ret_ty_for_return = handler_ret_ty.clone();
    let handler_fn = {
        let mut hb = LowerBuilder::new(
            &mut *builder.module,
            handler_fn_name.clone(),
            vec![],            // params déclarés manuellement ci-dessous
            handler_ret_ty,
        );
        hb.fn_ret_types  = builder.fn_ret_types.clone();
        hb.var_class     = builder.var_class.clone();
        hb.elem_types    = builder.elem_types.clone();
        hb.map_vars      = builder.map_vars.clone();
        hb.current_class = builder.current_class.clone();

        // Paramètres : err_val (i64) et err_type (i64)
        // On suit le même patron que lower_func :
        //   alloca slot  ← Store ← receiver (= block param Cranelift)
        let ev_slot = hb.declare_local("__err_val",  IrType::I64, false);
        let ev_recv = hb.new_value();
        hb.emit(Inst::Store { ptr: ev_slot.clone(), src: ev_recv.clone() });

        let et_slot = hb.declare_local("__err_type", IrType::I64, false);
        let et_recv = hb.new_value();
        hb.emit(Inst::Store { ptr: et_slot.clone(), src: et_recv.clone() });

        // Mettre à jour la liste des params de la fonction IR
        hb.func.params = vec![
            IrParam { name: "err_val".into(),  ty: IrType::I64, slot: ev_recv },
            IrParam { name: "err_type".into(), ty: IrType::I64, slot: et_recv },
        ];

        let end_bb = hb.new_block();

        for handler in handlers {
            let handler_bb = hb.new_block();
            let next_bb    = hb.new_block();

            if let Some(class_filter) = &handler.class_filter {
                // Charge err_type depuis son slot
                let et = hb.new_value();
                hb.emit(Inst::Load { dest: et.clone(), ptr: et_slot.clone(), ty: IrType::I64 });

                // Chaîne constante du filtre de classe
                let filter_idx = hb.module.intern_string(class_filter);
                let filter_val = hb.new_value();
                hb.emit(Inst::ConstStr { dest: filter_val.clone(), idx: filter_idx });

                // Appel __ocara_type_matches
                let match_result = hb.new_value();
                hb.emit(Inst::Call {
                    dest:   Some(match_result.clone()),
                    func:   "__ocara_type_matches".into(),
                    args:   vec![et, filter_val],
                    ret_ty: IrType::I64,
                });

                hb.emit(Inst::Branch {
                    cond:    match_result,
                    then_bb: handler_bb.clone(),
                    else_bb: next_bb.clone(),
                });
            } else {
                // Catch-all : saute directement dans le gestionnaire
                hb.emit(Inst::Jump { target: handler_bb.clone() });
            }

            // Bloc du gestionnaire
            hb.switch_to(&handler_bb);

            // Lie le binding à err_val
            let ev = hb.new_value();
            hb.emit(Inst::Load { dest: ev.clone(), ptr: ev_slot.clone(), ty: IrType::I64 });
            let e_slot = hb.declare_local(&handler.binding, IrType::I64, false);
            hb.emit(Inst::Store { ptr: e_slot, src: ev });

            // Associer le binding à la classe pour l'accès aux champs
            // Si filtre connu → utiliser le filtre, sinon → Exception par défaut
            let exception_class = handler.class_filter.clone().unwrap_or_else(|| "Exception".to_string());
            hb.var_class.insert(handler.binding.clone(), exception_class);

            lower_block(&mut hb, &handler.body);
            if !hb.is_terminated() {
                hb.emit(Inst::Jump { target: end_bb.clone() });
            }

            // Bloc suivant (quand le type ne correspond pas)
            hb.switch_to(&next_bb);
        }

        // Aucun handler ne correspond → re-propager l'erreur
        {
            let ev = hb.new_value();
            hb.emit(Inst::Load { dest: ev.clone(), ptr: ev_slot.clone(), ty: IrType::I64 });
            let et = hb.new_value();
            hb.emit(Inst::Load { dest: et.clone(), ptr: et_slot.clone(), ty: IrType::I64 });
            hb.emit(Inst::Call {
                dest:   None,
                func:   "__ocara_fail".into(),
                args:   vec![ev, et],
                ret_ty: IrType::Void,
            });
            // Après __ocara_fail, code mort mais l'IR doit être terminé
            let ret_val = if handler_ret_ty_for_return != IrType::Void {
                let dummy = hb.new_value();
                hb.emit(Inst::ConstInt { dest: dummy.clone(), value: 0 });
                Some(dummy)
            } else {
                None
            };
            hb.emit(Inst::Return { value: ret_val });
        }

        hb.switch_to(&end_bb);
        // Return implicite : si aucun handler n'a fait return explicite
        let ret_val = if handler_ret_ty_for_return != IrType::Void {
            let dummy = hb.new_value();
            hb.emit(Inst::ConstInt { dest: dummy.clone(), value: 0 });
            Some(dummy)
        } else {
            None
        };
        hb.emit(Inst::Return { value: ret_val });

        hb.func   // move func out, drops hb, releases module reborrow
    };
    builder.module.add_function(handler_fn);

    // ── 3. Appel du trampoline dans la fonction englobante ───────────────────
    let body_addr = builder.new_value();
    builder.emit(Inst::FuncAddr { dest: body_addr.clone(), func: body_fn_name });

    let handler_addr = builder.new_value();
    builder.emit(Inst::FuncAddr { dest: handler_addr.clone(), func: handler_fn_name });

    let try_result = builder.new_value();
    
    if captures.is_empty() {
        // Pas de captures : utiliser __ocara_try_exec
        builder.emit(Inst::Call {
            dest:   Some(try_result.clone()),
            func:   "__ocara_try_exec".into(),
            args:   vec![body_addr, handler_addr],
            ret_ty: IrType::I64,
        });
    } else {
        // Avec captures : allouer un tableau sur le tas et passer son adresse
        
        // Calculer la taille du tableau : num_captures * 8 bytes
        let num_captures = captures.len();
        let size_bytes = (num_captures * 8) as i64;
        
        // Allouer le tableau sur le tas avec __alloc_obj
        let size_val = builder.new_value();
        builder.emit(Inst::ConstInt { dest: size_val.clone(), value: size_bytes });
        
        let array_ptr = builder.new_value();
        builder.emit(Inst::Call {
            dest: Some(array_ptr.clone()),
            func: "__alloc_obj".into(),
            args: vec![size_val],
            ret_ty: IrType::Ptr,
        });
        
        // Stocker chaque capture dans le tableau
        for (idx, (name, _ty)) in captures.iter().enumerate() {
            // Charger la valeur depuis locals
            let val = if let Some((slot, slot_ty, _)) = builder.locals.get(name).cloned() {
                let v = builder.new_value();
                builder.emit(Inst::Load { dest: v.clone(), ptr: slot, ty: slot_ty });
                v
            } else {
                // Capture non trouvée dans locals - ne devrait pas arriver
                let v = builder.new_value();
                builder.emit(Inst::ConstInt { dest: v.clone(), value: 0 });
                v
            };
            
            // Calculer l'adresse de l'élément : array_ptr + idx*8
            let offset = builder.new_value();
            builder.emit(Inst::ConstInt { dest: offset.clone(), value: (idx * 8) as i64 });
            
            let elem_addr = builder.new_value();
            builder.emit(Inst::Add {
                dest: elem_addr.clone(),
                lhs: array_ptr.clone(),
                rhs: offset,
                ty: IrType::I64,
            });
            
            // Stocker la valeur à cette adresse
            builder.emit(Inst::Store { ptr: elem_addr, src: val });
        }
        
        // Appeler __ocara_try_exec_with_captures avec le pointeur vers le tableau
        builder.emit(Inst::Call {
            dest:   Some(try_result.clone()),
            func:   "__ocara_try_exec_with_captures".into(),
            args:   vec![body_addr, handler_addr, array_ptr],
            ret_ty: IrType::I64,
        });
    }
    
    // Vérifier si le handler a fait un return (try_result != 0)
    // Si oui, extraire la valeur (try_result - 1) et retourner immédiatement
    let continue_bb = builder.new_block();
    let return_bb = builder.new_block();
    
    // Comparaison explicite : try_result != 0
    let zero = builder.new_value();
    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    
    let has_return = builder.new_value();
    builder.emit(Inst::CmpNe {
        dest: has_return.clone(),
        lhs: try_result.clone(),
        rhs: zero,
        ty: IrType::I64,
    });
    
    builder.emit(Inst::Branch {
        cond: has_return,
        then_bb: return_bb.clone(),
        else_bb: continue_bb.clone(),
    });
    
    // Bloc return : extraire la valeur (try_result - 1) et retourner
    builder.switch_to(&return_bb);
    let one = builder.new_value();
    builder.emit(Inst::ConstInt { dest: one.clone(), value: 1 });
    
    let return_value = builder.new_value();
    builder.emit(Inst::Sub {
        dest: return_value.clone(),
        lhs: try_result,
        rhs: one,
        ty: IrType::I64,
    });
    
    // Si on est dans la fonction main (bloc runtime), transformer le return
    // en assignation ERROR puis sauter à la fin du bloc main
    if builder.func.name == "main" {
        // Charger le slot de ERROR (doit exister)
        if let Some((error_slot, _, _)) = builder.locals.get("ERROR").cloned() {
            // ERROR = return_value
            builder.emit(Inst::Store { ptr: error_slot.clone(), src: return_value.clone() });
            
            // Sauter au label de sortie anticipée du bloc main (avant if ERROR != 0)
            if let Some(exit_bb) = builder.runtime_exit_bb.clone() {
                builder.emit(Inst::Jump { target: exit_bb });
            } else {
                builder.emit(Inst::Jump { target: continue_bb.clone() });
            }
        } else {
            // ERROR n'existe pas (ne devrait pas arriver), faire un return normal
            builder.emit(Inst::Return { value: Some(return_value) });
        }
    } else {
        // Fonction normale : vrai return
        builder.emit(Inst::Return { value: Some(return_value) });
    }
    
    // Bloc continue : continuer l'exécution normale
    // DOIT TOUJOURS être défini car il est référencé dans le Branch
    builder.switch_to(&continue_bb);
}
