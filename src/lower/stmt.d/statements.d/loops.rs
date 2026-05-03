/// Lowering des boucles (for in, for map, break, continue)

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::ir::inst::Inst;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::lower_expr;
use super::super::super::block::lower_block;

pub fn lower_for_in(
    builder: &mut LowerBuilder,
    var: &str,
    iter: &Expr,
    body: &Block,
) {
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

pub fn lower_for_map(
    builder: &mut LowerBuilder,
    key: &str,
    value: &str,
    iter: &Expr,
    body: &Block,
) {
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

pub fn lower_break(builder: &mut LowerBuilder) {
    if let Some((_, break_bb)) = builder.loop_stack.last().cloned() {
        builder.emit(Inst::Jump { target: break_bb });
    }
}

pub fn lower_continue(builder: &mut LowerBuilder) {
    if let Some((continue_bb, _)) = builder.loop_stack.last().cloned() {
        builder.emit(Inst::Jump { target: continue_bb });
    }
}
