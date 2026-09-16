/// Lowering des boucles (for in, for map, break, continue)

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::ir::inst::Inst;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::{lower_expr, hoist_closure_promotions_before_loop};
use super::super::super::block::lower_block;

pub fn lower_for_in(
    builder: &mut LowerBuilder,
    var: &str,
    iter: &Expr,
    body: &Block,
) {
    // `for x in truc(...)` où `truc` est un générateur (`emit`/`message<T>`,
    // voir docs/roadmap.d/langage-emit-iterable.md) : lowering entièrement
    // différent (machine à états, pas de tableau) — voir
    // `crate::lower::builder::message_gen::lower_for_message`.
    if let Some((mangled, elem_ty)) = crate::lower::builder::message_gen::detect_message_call(builder, iter) {
        crate::lower::builder::message_gen::lower_for_message(builder, var, iter, &mangled, elem_ty, body);
        return;
    }

    // Pré-promotion : voir docs/roadmap.d/langage-closure-promotion-in-loop.md.
    // AVANT de déclarer `var` (la variable d'itération elle-même n'existe pas
    // encore ici, donc jamais concernée par ce pré-scan — seules les
    // variables déjà existantes AVANT la boucle le sont).
    hoist_closure_promotions_before_loop(builder, body);

    // Lowering : __iter_init(iter), boucle sur __iter_next
    let iter_val  = lower_expr(builder, iter);
    let idx_slot  = builder.declare_local("__for_idx", IrType::I64, true);
    let zero = builder.new_value();
    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    builder.emit(Inst::Store { ptr: idx_slot.clone(), src: zero });

    // Type de l'élément : I64 pour les plages entières, Ptr pour les tableaux
    let elem_ty = match iter {
        Expr::Range { .. } => IrType::I64,
        Expr::Ident(name, _) => {
            builder.elem_types.get(name.as_str()).cloned().unwrap_or(IrType::Ptr)
        }
        _ => IrType::Ptr,
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
    
    // Pour les paramètres variadic, le tableau IR est Ptr (mixed[]) donc on doit traiter
    // différemment : récupérer comme I64 puis caster/unboxer si nécessaire
    let is_variadic = if let Expr::Ident(name, _) = iter {
        builder.variadic_params.contains(name.as_str())
    } else {
        false
    };
    
    if is_variadic {
        // Variadic : le tableau est mixed[], donc __array_get retourne un i64 brut
        builder.emit(Inst::Call {
            dest:   Some(elem.clone()),
            func:   "__array_get".into(),
            args:   vec![iter_val.clone(), idx.clone()],
            ret_ty: IrType::I64,  // Le tableau mixed contient des i64
        });
        // Les int sont déjà corrects en i64, pas besoin d'unboxing
        // Les float/bool nécessiteraient unboxing mais pour l'instant on les laisse
    } else {
        // Tableau normal : utiliser le type d'élément
        builder.emit(Inst::Call {
            dest:   Some(elem.clone()),
            func:   "__array_get".into(),
            args:   vec![iter_val.clone(), idx.clone()],
            ret_ty: elem_ty.clone(),
        });
    }
    
    builder.declare_local(var, elem_ty.clone(), false);
    builder.store_local(var, elem);
    
    // Si l'itérateur est une variable avec un type d'élément map, enregistrer les métadonnées
    if let Expr::Ident(iter_name, _) = iter {
        if let Some(elem_ast_ty) = builder.elem_ast_types.get(iter_name.as_str()) {
            if let Type::Map(_, val_ty) = elem_ast_ty {
                // L'élément est un map, enregistrer la variable d'itération comme map
                builder.map_vars.insert(var.to_string());
                builder.elem_types.insert(var.to_string(), IrType::from_ast(val_ty));
                builder.var_class.insert(var.to_string(), "Map".to_string());
            }
        }
    }

    // continue → incr_bb, break → merge_bb
    builder.loop_stack.push((incr_bb.clone(), merge_bb.clone(), builder.block_scope_stack.len()));
    builder.loop_depth += 1;
    lower_block(builder, body);
    builder.loop_depth -= 1;
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

pub fn lower_for_map(
    builder: &mut LowerBuilder,
    key: &str,
    value: &str,
    iter: &Expr,
    body: &Block,
) {
    // Pré-promotion : voir docs/roadmap.d/langage-closure-promotion-in-loop.md.
    hoist_closure_promotions_before_loop(builder, body);

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
    builder.declare_local(key, IrType::Ptr, false);
    builder.store_local(key, k.clone());

    // Valeur correspondante
    let v = builder.new_value();
    builder.emit(Inst::Call {
        dest:   Some(v.clone()),
        func:   "__map_get".into(),
        args:   vec![iter_val.clone(), k],
        ret_ty: IrType::I64,
    });
    builder.declare_local(value, IrType::I64, false);
    builder.store_local(value, v);

    // continue → incr_bb, break → merge_bb
    builder.loop_stack.push((incr_bb.clone(), merge_bb.clone(), builder.block_scope_stack.len()));
    builder.loop_depth += 1;
    lower_block(builder, body);
    builder.loop_depth -= 1;
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

pub fn lower_break(builder: &mut LowerBuilder) {
    if let Some((_, break_bb, depth)) = builder.loop_stack.last().cloned() {
        // Détruit les scoped/consumed encore vivantes entre ici et l'entrée
        // de la boucle (corps de boucle inclus) avant de sauter dehors —
        // voir crate::lower::stmt::ownership::emit_early_exit_drops.
        crate::lower::stmt::ownership::emit_early_exit_drops(builder, depth);
        builder.emit(Inst::Jump { target: break_bb });
    }
}

pub fn lower_continue(builder: &mut LowerBuilder) {
    if let Some((continue_bb, _, depth)) = builder.loop_stack.last().cloned() {
        // Même destruction que `break` : `continue` quitte aussi le corps
        // de boucle actuellement ouvert (et tout ce qu'il contient), juste
        // pour reboucler plutôt que sortir complètement.
        crate::lower::stmt::ownership::emit_early_exit_drops(builder, depth);
        builder.emit(Inst::Jump { target: continue_bb });
    }
}
