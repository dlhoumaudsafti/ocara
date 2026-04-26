use crate::ast::*;
use crate::ir::func::IrParam;
use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::{lower_expr, expr_ir_type_pub};

// ─────────────────────────────────────────────────────────────────────────────
// Lowering de blocs et d'instructions
// ─────────────────────────────────────────────────────────────────────────────

/// Si la variable cible est de type `mixed` (Ptr) et la valeur est F64 ou Bool,
/// on la boxe pour éviter que les bits soient interprétés comme un pointeur.
fn box_for_any(builder: &mut LowerBuilder, target_ty: &IrType, val_ty: IrType, val: Value) -> Value {
    if *target_ty != IrType::Ptr { return val; }
    match val_ty {
        IrType::F64 => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_float".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        IrType::Bool => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_bool".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        _ => val,
    }
}

pub fn lower_block(builder: &mut LowerBuilder, block: &Block) {
    for stmt in &block.stmts {
        if builder.is_terminated() { break; }
        lower_stmt(builder, stmt);
    }
}

pub fn lower_stmt(builder: &mut LowerBuilder, stmt: &Stmt) {
    match stmt {
        // ── Déclaration de variable ───────────────────────────────────────────
        Stmt::Var { name, ty, value, mutable, .. } => {
            let ir_ty = IrType::from_ast(ty);
            // Si c'est un tableau, enregistrer le type des éléments
            if let crate::ast::Type::Array(inner) = ty {
                builder.elem_types.insert(name.clone(), IrType::from_ast(inner));
            }
            // Si c'est une map, marquer la variable pour Expr::Index → __map_get
            // et enregistrer le type des valeurs dans elem_types
            if let crate::ast::Type::Map(_, val_ty) = ty {
                builder.map_vars.insert(name.clone());
                builder.elem_types.insert(name.clone(), IrType::from_ast(val_ty));
            }
            // Si c'est un type de classe, enregistrer le mapping var → classe
            if let crate::ast::Type::Named(class_name) = ty {
                builder.var_class.insert(name.clone(), class_name.clone());
            }
            let _slot = builder.declare_local(name, ir_ty.clone(), *mutable);
            let val_ty = expr_ir_type_pub(builder, value);
            let val = lower_expr(builder, value);
            let val = box_for_any(builder, &ir_ty, val_ty, val);
            builder.store_local(name, val);
        }

        Stmt::Const { name, ty, value, .. } => {
            let ir_ty = IrType::from_ast(ty);
            if let crate::ast::Type::Array(inner) = ty {
                builder.elem_types.insert(name.clone(), IrType::from_ast(inner));
            }
            if let crate::ast::Type::Map(_, val_ty) = ty {
                builder.map_vars.insert(name.clone());
                builder.elem_types.insert(name.clone(), IrType::from_ast(val_ty));
            }
            if let crate::ast::Type::Named(class_name) = ty {
                builder.var_class.insert(name.clone(), class_name.clone());
            }
            let _slot = builder.declare_local(name, ir_ty.clone(), false);
            let val_ty = expr_ir_type_pub(builder, value);
            let val = lower_expr(builder, value);
            let val = box_for_any(builder, &ir_ty, val_ty, val);
            builder.store_local(name, val);
        }

        // ── Expression seule ──────────────────────────────────────────────────
        Stmt::Expr(expr) => {
            lower_expr(builder, expr);
        }

        // ── Return ────────────────────────────────────────────────────────────
        Stmt::Return { value, .. } => {
            let v = value.as_ref().map(|e| lower_expr(builder, e));
            builder.emit(Inst::Return { value: v });
        }

        // ── If / elseif / else ────────────────────────────────────────────────
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            let cond_val = lower_expr(builder, condition);
            let then_bb  = builder.new_block();
            let else_bb  = builder.new_block();
            let merge_bb = builder.new_block();

            builder.emit(Inst::Branch {
                cond:    cond_val,
                then_bb: then_bb.clone(),
                else_bb: else_bb.clone(),
            });

            // Then
            builder.switch_to(&then_bb);
            lower_block(builder, then_block);
            if !builder.is_terminated() {
                builder.emit(Inst::Jump { target: merge_bb.clone() });
            }

            // Elseif / Else
            builder.switch_to(&else_bb);
            if !elseif.is_empty() {
                // Lowering de la chaîne elseif de manière récursive
                lower_elseif_chain(builder, elseif, else_block.as_ref(), &merge_bb);
            } else if let Some(blk) = else_block {
                lower_block(builder, blk);
                if !builder.is_terminated() {
                    builder.emit(Inst::Jump { target: merge_bb.clone() });
                }
            } else {
                builder.emit(Inst::Jump { target: merge_bb.clone() });
            }

            builder.switch_to(&merge_bb);
        }

        // ── Switch ────────────────────────────────────────────────────────────
        Stmt::Switch { subject, cases, default, .. } => {
            let subj = lower_expr(builder, subject);
            let merge_bb = builder.new_block();

            for case in cases {
                let body_bb = builder.new_block();
                let next_bb = builder.new_block();

                let pat_val = lower_expr(
                    builder,
                    &Expr::Literal(case.pattern.clone(), case.span.clone()),
                );
                let test = builder.new_value();
                builder.emit(Inst::CmpEq {
                    dest: test.clone(),
                    lhs:  subj.clone(),
                    rhs:  pat_val,
                    ty:   IrType::I64,
                });
                builder.emit(Inst::Branch {
                    cond:    test,
                    then_bb: body_bb.clone(),
                    else_bb: next_bb.clone(),
                });

                builder.switch_to(&body_bb);
                lower_block(builder, &case.body);
                if !builder.is_terminated() {
                    builder.emit(Inst::Jump { target: merge_bb.clone() });
                }

                builder.switch_to(&next_bb);
            }

            if let Some(blk) = default {
                lower_block(builder, blk);
            }
            if !builder.is_terminated() {
                builder.emit(Inst::Jump { target: merge_bb.clone() });
            }

            builder.switch_to(&merge_bb);
        }

        // ── While ─────────────────────────────────────────────────────────────
        Stmt::While { condition, body, .. } => {
            let cond_bb  = builder.new_block();
            let body_bb  = builder.new_block();
            let merge_bb = builder.new_block();

            builder.emit(Inst::Jump { target: cond_bb.clone() });
            builder.switch_to(&cond_bb);

            let cond_val = lower_expr(builder, condition);
            builder.emit(Inst::Branch {
                cond:    cond_val,
                then_bb: body_bb.clone(),
                else_bb: merge_bb.clone(),
            });

            builder.switch_to(&body_bb);
            // continue → cond_bb (réévalue la condition), break → merge_bb
            builder.loop_stack.push((cond_bb.clone(), merge_bb.clone()));
            lower_block(builder, body);
            builder.loop_stack.pop();
            if !builder.is_terminated() {
                builder.emit(Inst::Jump { target: cond_bb.clone() });
            }

            builder.switch_to(&merge_bb);
        }

        // ── For in ────────────────────────────────────────────────────────────
        Stmt::ForIn { var, iter, body, .. } => {
            // Lowering : __iter_init(iter), boucle sur __iter_next
            let iter_val  = lower_expr(builder, iter);
            let idx_slot  = builder.declare_local("__for_idx", IrType::I64, true);
            let zero = builder.new_value();
            builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
            builder.emit(Inst::Store { ptr: idx_slot.clone(), src: zero });

            // Type de l'élément : I64 pour les plages entières, Ptr pour les tableaux
            let elem_ty = match iter {
                crate::ast::Expr::Range { .. } => IrType::I64,                crate::ast::Expr::Ident(name, _) => {
                    builder.elem_types.get(name.as_str()).cloned().unwrap_or(IrType::Ptr)
                }                _ => IrType::Ptr,
            };

            // Longueur du tableau
            let len_val = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(len_val.clone()),
                func:   "__array_len".into(),
                args:   vec![iter_val.clone()],
                ret_ty: IrType::I64,
            });

            let cond_bb  = builder.new_block();
            let body_bb  = builder.new_block();
            let incr_bb  = builder.new_block();
            let merge_bb = builder.new_block();

            builder.emit(Inst::Jump { target: cond_bb.clone() });
            builder.switch_to(&cond_bb);

            let idx = builder.new_value();
            builder.emit(Inst::Load { dest: idx.clone(), ptr: idx_slot.clone(), ty: IrType::I64 });
            let cond = builder.new_value();
            builder.emit(Inst::CmpLt {
                dest: cond.clone(),
                lhs:  idx.clone(),
                rhs:  len_val.clone(),
                ty:   IrType::I64,
            });
            builder.emit(Inst::Branch {
                cond:    cond,
                then_bb: body_bb.clone(),
                else_bb: merge_bb.clone(),
            });

            builder.switch_to(&body_bb);
            // Charge l'élément courant
            let elem = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(elem.clone()),
                func:   "__array_get".into(),
                args:   vec![iter_val.clone(), idx.clone()],
                ret_ty: elem_ty.clone(),
            });
            let elem_slot = builder.declare_local(var, elem_ty, false);
            builder.emit(Inst::Store { ptr: elem_slot, src: elem });

            // continue → incr_bb, break → merge_bb
            builder.loop_stack.push((incr_bb.clone(), merge_bb.clone()));
            lower_block(builder, body);
            builder.loop_stack.pop();

            if !builder.is_terminated() {
                builder.emit(Inst::Jump { target: incr_bb.clone() });
            }

            // Bloc incrément
            builder.switch_to(&incr_bb);
            let one = builder.new_value();
            builder.emit(Inst::ConstInt { dest: one.clone(), value: 1 });
            let idx2 = builder.new_value();
            builder.emit(Inst::Load { dest: idx2.clone(), ptr: idx_slot.clone(), ty: IrType::I64 });
            let next_idx = builder.new_value();
            builder.emit(Inst::Add { dest: next_idx.clone(), lhs: idx2, rhs: one, ty: IrType::I64 });
            builder.emit(Inst::Store { ptr: idx_slot, src: next_idx });
            builder.emit(Inst::Jump { target: cond_bb.clone() });

            builder.switch_to(&merge_bb);
        }

        // ── For map ───────────────────────────────────────────────────────────
        Stmt::ForMap { key, value, iter, body, .. } => {
            let iter_val = lower_expr(builder, iter);

            // Récupère le tableau des clés
            let keys_arr = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(keys_arr.clone()),
                func:   "Map_keys".into(),
                args:   vec![iter_val.clone()],
                ret_ty: IrType::Ptr,
            });

            // Longueur
            let len_val = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(len_val.clone()),
                func:   "__array_len".into(),
                args:   vec![keys_arr.clone()],
                ret_ty: IrType::I64,
            });

            // Index
            let idx_slot = builder.declare_local("__map_idx", IrType::I64, true);
            let zero = builder.new_value();
            builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
            builder.emit(Inst::Store { ptr: idx_slot.clone(), src: zero });

            let cond_bb  = builder.new_block();
            let body_bb  = builder.new_block();
            let incr_bb  = builder.new_block();
            let merge_bb = builder.new_block();

            builder.emit(Inst::Jump { target: cond_bb.clone() });
            builder.switch_to(&cond_bb);

            let idx = builder.new_value();
            builder.emit(Inst::Load { dest: idx.clone(), ptr: idx_slot.clone(), ty: IrType::I64 });
            let cond = builder.new_value();
            builder.emit(Inst::CmpLt {
                dest: cond.clone(), lhs: idx.clone(), rhs: len_val.clone(), ty: IrType::I64,
            });
            builder.emit(Inst::Branch { cond, then_bb: body_bb.clone(), else_bb: merge_bb.clone() });

            builder.switch_to(&body_bb);

            // Clé courante
            let k = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(k.clone()),
                func:   "__array_get".into(),
                args:   vec![keys_arr.clone(), idx.clone()],
                ret_ty: IrType::Ptr,
            });
            let key_slot = builder.declare_local(key, IrType::Ptr, false);
            builder.emit(Inst::Store { ptr: key_slot, src: k.clone() });

            // Valeur correspondante
            let v = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(v.clone()),
                func:   "__map_get".into(),
                args:   vec![iter_val.clone(), k],
                ret_ty: IrType::I64,
            });
            let val_slot = builder.declare_local(value, IrType::I64, false);
            builder.emit(Inst::Store { ptr: val_slot, src: v });

            // continue → incr_bb, break → merge_bb
            builder.loop_stack.push((incr_bb.clone(), merge_bb.clone()));
            lower_block(builder, body);
            builder.loop_stack.pop();

            if !builder.is_terminated() {
                builder.emit(Inst::Jump { target: incr_bb.clone() });
            }

            // Bloc incrément
            builder.switch_to(&incr_bb);
            let one = builder.new_value();
            builder.emit(Inst::ConstInt { dest: one.clone(), value: 1 });
            let idx2 = builder.new_value();
            builder.emit(Inst::Load { dest: idx2.clone(), ptr: idx_slot.clone(), ty: IrType::I64 });
            let next_idx = builder.new_value();
            builder.emit(Inst::Add { dest: next_idx.clone(), lhs: idx2, rhs: one, ty: IrType::I64 });
            builder.emit(Inst::Store { ptr: idx_slot, src: next_idx });
            builder.emit(Inst::Jump { target: cond_bb.clone() });

            builder.switch_to(&merge_bb);
        }

        // ── Break ─────────────────────────────────────────────────────────────
        Stmt::Break { .. } => {
            if let Some((_, break_bb)) = builder.loop_stack.last().cloned() {
                builder.emit(Inst::Jump { target: break_bb });
            }
        }

        // ── Continue ──────────────────────────────────────────────────────────
        Stmt::Continue { .. } => {
            if let Some((continue_bb, _)) = builder.loop_stack.last().cloned() {
                builder.emit(Inst::Jump { target: continue_bb });
            }
        }

        // ── Affectation : `target = value` ───────────────────────────────────
        Stmt::Assign { target, value, .. } => {
            let val_ty = expr_ir_type_pub(builder, value);
            let val = lower_expr(builder, value);
            match target {
                Expr::Ident(name, _) => {
                    // Boxing si la variable cible est mixed
                    let target_ty = builder.locals.get(name.as_str())
                        .map(|(_, ty, _)| ty.clone())
                        .unwrap_or(IrType::I64);
                    let val = box_for_any(builder, &target_ty, val_ty, val);
                    builder.store_local(name, val);
                }
                Expr::Field { object, field, .. } => {
                    // Calculer l'offset du champ
                    let class_name = match object.as_ref() {
                        Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                        Expr::SelfExpr(_)    => builder.current_class.clone(),
                        _ => None,
                    };
                    let offset = if let Some(cls) = &class_name {
                        if let Some(fields) = builder.module.class_layouts.get(cls.as_str()) {
                            let off = fields.iter().position(|(f, _)| f == field).unwrap_or(0) as i32 * 8;
                            off
                        } else { 0 }
                    } else { 0 };
                    let obj_val = lower_expr(builder, object);
                    builder.emit(Inst::SetField {
                        obj:   obj_val,
                        field: field.clone(),
                        src:   val,
                        offset,
                    });
                }
                Expr::Index { object, index, .. } => {
                    let obj_val = lower_expr(builder, object);
                    let idx_val = lower_expr(builder, index);
                    builder.emit(Inst::Call {
                        dest:   None,
                        func:   "__array_set".into(),
                        args:   vec![obj_val, idx_val, val],
                        ret_ty: IrType::Void,
                    });
                }
                _ => {
                    // cible invalide — ignorée silencieusement (sema a déjà rapporté l'erreur)
                }
            }
        }

        // ── Raise ────────────────────────────────────────────────────────────
        Stmt::Raise { value, .. } => {
            lower_raise(builder, value);
        }

        // ── Try / On ─────────────────────────────────────────────────────────
        Stmt::Try { body, handlers, .. } => {
            lower_try(builder, body, handlers);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Chaîne elseif récursive
// ─────────────────────────────────────────────────────────────────────────────

fn lower_elseif_chain(
    builder:    &mut LowerBuilder,
    elseif:     &[(Expr, Block)],
    else_block: Option<&Block>,
    merge_bb:   &crate::ir::inst::BlockId,
) {
    if elseif.is_empty() {
        if let Some(blk) = else_block {
            lower_block(builder, blk);
        }
        if !builder.is_terminated() {
            builder.emit(Inst::Jump { target: merge_bb.clone() });
        }
        return;
    }

    let (cond_expr, then_blk) = &elseif[0];
    let cond_val = lower_expr(builder, cond_expr);
    let then_bb  = builder.new_block();
    let next_bb  = builder.new_block();

    builder.emit(Inst::Branch {
        cond:    cond_val,
        then_bb: then_bb.clone(),
        else_bb: next_bb.clone(),
    });

    builder.switch_to(&then_bb);
    lower_block(builder, then_blk);
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: merge_bb.clone() });
    }

    builder.switch_to(&next_bb);
    lower_elseif_chain(builder, &elseif[1..], else_block, merge_bb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Lowering de `raise expr`
// ─────────────────────────────────────────────────────────────────────────────

fn lower_raise(builder: &mut LowerBuilder, value: &Expr) {
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

// ─────────────────────────────────────────────────────────────────────────────
// Lowering de `try { body } on e [is Foo] { handler } …`
//
// Modèle callback :
//   1. Le corps try      → fonction __try_body_N  () -> void
//   2. Le gestionnaire   → fonction __try_handler_N (err_val:i64, err_type:i64) -> void
//   3. Dans la fonction englobante :
//        body_addr    = FuncAddr(__try_body_N)
//        handler_addr = FuncAddr(__try_handler_N)
//        call __ocara_try_exec(body_addr, handler_addr)
//
// __ocara_try_exec (C) fait le setjmp, appelle le corps, et en cas d'erreur
// appelle le gestionnaire.  La frame de __ocara_try_exec reste vivante pendant
// tout l'exécution du corps, ce qui garantit la validité du jmp_buf.
// ─────────────────────────────────────────────────────────────────────────────

fn lower_try(builder: &mut LowerBuilder, body: &Block, handlers: &[OnClause]) {
    // ID unique fondé sur le nombre de fonctions déjà dans le module
    let try_id = builder.module.functions.len();
    let body_fn_name    = format!("__try_body_{}", try_id);
    let handler_fn_name = format!("__try_handler_{}", try_id);

    // ── 1. Corps try ─────────────────────────────────────────────────────────
    let body_fn = {
        let mut bb = LowerBuilder::new(
            &mut *builder.module,
            body_fn_name.clone(),
            vec![],
            IrType::Void,
        );
        bb.fn_ret_types  = builder.fn_ret_types.clone();
        bb.var_class     = builder.var_class.clone();
        bb.elem_types    = builder.elem_types.clone();
        bb.map_vars      = builder.map_vars.clone();
        bb.current_class = builder.current_class.clone();

        lower_block(&mut bb, body);
        if !bb.is_terminated() {
            bb.emit(Inst::Return { value: None });
        }
        bb.func   // move func out, drops bb, releases module reborrow
    };
    builder.module.add_function(body_fn);

    // ── 2. Gestionnaire ──────────────────────────────────────────────────────
    let handler_fn = {
        let mut hb = LowerBuilder::new(
            &mut *builder.module,
            handler_fn_name.clone(),
            vec![],            // params déclarés manuellement ci-dessous
            IrType::Void,
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

            // Si filtre connu → associer le binding à la classe pour l'accès aux champs
            if let Some(class_filter) = &handler.class_filter {
                hb.var_class.insert(handler.binding.clone(), class_filter.clone());
            }

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
            hb.emit(Inst::Return { value: None });
        }

        hb.switch_to(&end_bb);
        hb.emit(Inst::Return { value: None });

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
