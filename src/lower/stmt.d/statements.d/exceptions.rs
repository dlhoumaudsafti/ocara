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
    // ID unique fondé sur le nombre de fonctions déjà dans le module
    let try_id = builder.module.functions.len();
    let body_fn_name    = format!("__try_body_{}", try_id);
    let handler_fn_name = format!("__try_handler_{}", try_id);

    // ── 1. Corps try ─────────────────────────────────────────────────────────
    // Le body doit avoir le même type de retour que la fonction englobante
    // car il peut contenir des return qui remontent à la fonction parente
    let body_ret_ty = builder.func.ret_ty.clone();
    let body_fn = {
        let mut bb = LowerBuilder::new(
            &mut *builder.module,
            body_fn_name.clone(),
            vec![],
            body_ret_ty.clone(),
        );
        bb.fn_ret_types  = builder.fn_ret_types.clone();
        bb.var_class     = builder.var_class.clone();
        bb.elem_types    = builder.elem_types.clone();
        bb.map_vars      = builder.map_vars.clone();
        bb.current_class = builder.current_class.clone();

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

    builder.emit(Inst::Call {
        dest:   None,
        func:   "__ocara_try_exec".into(),
        args:   vec![body_addr, handler_addr],
        ret_ty: IrType::Void,
    });
}
