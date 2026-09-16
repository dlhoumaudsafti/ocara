/// Lowering des affectations

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::ir::inst::Inst;
use crate::lower::builder::LowerBuilder;
use crate::ir::inst::Value;
use crate::lower::expr::{lower_expr, expr_ir_type_pub};
use super::helpers::box_for_any;
use crate::lower::expr::helpers::{resolve_chained_field_class, is_map_target, field_offset, field_ir_type};

pub fn lower_assign(
    builder: &mut LowerBuilder,
    target: &Expr,
    value: &Expr,
) {
    let val_ty = expr_ir_type_pub(builder, value);
    let val = lower_expr(builder, value);
    // `target = value` : `value` peut être une `scoped`/`consumed` qui
    // s'échappe vers `target` (voir crate::lower::stmt::ownership).
    let val = crate::lower::stmt::ownership::maybe_clone_escaping(builder, value, val);

    match target {
        Expr::Ident(name, _) => {
            // Boxing si la variable cible est mixed
            let target_ty = builder.frame_vars.get(name.as_str())
                .map(|(_, _, ty)| ty.clone())
                .or_else(|| builder.locals.get(name.as_str()).map(|(_, ty, _)| ty.clone()))
                .unwrap_or(IrType::I64);
            let val = box_for_any(builder, &target_ty, val_ty, val);
            // `s = nouvelleValeur` où `s` est `scoped`/`consumed` : libérer
            // l'ancienne valeur avant de la remplacer, sinon elle fuit (voir
            // crate::lower::stmt::ownership::free_before_reassign).
            crate::lower::stmt::ownership::free_before_reassign(builder, name);
            builder.store_local(name, val);
        }
        Expr::Field { object, field, .. } => {
            // Calculer l'offset du champ
            let class_name = match object.as_ref() {
                Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                Expr::SelfExpr(_)    => builder.current_class.clone(),
                // Accès chaîné (`w.inner.x = ...`) — voir
                // resolve_chained_field_class pour le bug historique corrigé.
                Expr::Field { object: inner, field: inner_field, .. } => {
                    resolve_chained_field_class(builder, inner, inner_field)
                }
                _ => None,
            };
            let offset = class_name.as_deref()
                .map(|cls| field_offset(&builder.module.class_layouts, cls, field))
                .unwrap_or(0);
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
            // Même détection map-vs-array que la lecture (Expr::Index dans
            // lower.rs, et lower_incdec ci-dessous) — sans elle, `map[clé] = v`
            // (variable OU self.champ) appelait toujours __array_set, qui
            // réinterprète le pointeur de map comme un tableau et corrompt sa
            // structure interne (crash au premier accès/itération suivant,
            // silencieux avant ça). Factorisé dans `is_map_target` (helpers.rs).
            let is_map = is_map_target(builder, object);
            let func = if is_map { "__map_set" } else { "__array_set" };
            builder.emit(Inst::Call {
                dest:   None,
                func:   func.into(),
                args:   vec![obj_val, idx_val, val],
                ret_ty: IrType::Void,
            });
        }
        _ => {
            // cible invalide — ignorée silencieusement (sema a déjà rapporté l'erreur)
        }
    }
}

/// Lowering de `i++`/`++i`/`i--`/`--i` (`Expr::IncDec`, voir
/// docs/roadmap.d/langage-increment-decrement.md) : charge l'ancienne valeur
/// de `target`, calcule `ancienne ± 1`, la stocke, et retourne l'ancienne
/// valeur (forme suffixe) ou la nouvelle (forme préfixe).
///
/// `object`/`index` ne sont JAMAIS évalués plus d'une fois — `arr[calc()]++`
/// n'appelle `calc()` qu'une seule fois (la `Value` obtenue est réutilisée
/// pour la lecture ET l'écriture), contrairement à un enchaînement naïf
/// "lecture puis `lower_assign`" qui réévaluerait `object`/`index`.
///
/// Sema (`src/sema/typecheck.rs`, cas `Expr::IncDec`) a déjà validé que
/// `target` est un `Ident`/`Field`/`Index` de type `int`/`float` — jamais
/// `mixed`/`scoped`/`consumed` (aucun boxing ni libération à faire ici,
/// contrairement à `lower_assign`).
pub fn lower_incdec(builder: &mut LowerBuilder, op: &IncDecOp, target: &Expr) -> Value {
    match target {
        Expr::Ident(name, _) => {
            let (old_val, ty) = builder.load_local(name).unwrap_or_else(|| {
                let d = builder.new_value();
                builder.emit(Inst::Nop);
                (d, IrType::I64)
            });
            let new_val = emit_incdec_step(builder, op, old_val.clone(), &ty);
            builder.store_local(name, new_val.clone());
            if op.is_prefix() { new_val } else { old_val }
        }
        Expr::Field { object, field, .. } => {
            let class_name = match object.as_ref() {
                Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                Expr::SelfExpr(_)    => builder.current_class.clone(),
                Expr::Field { object: inner, field: inner_field, .. } => {
                    resolve_chained_field_class(builder, inner, inner_field)
                }
                _ => None,
            };
            let (offset, ty) = match &class_name {
                Some(cls) => (
                    field_offset(&builder.module.class_layouts, cls, field),
                    field_ir_type(&builder.module.class_layouts, cls, field),
                ),
                None => (0, IrType::I64),
            };
            let obj_val = lower_expr(builder, object);
            let old_val = builder.new_value();
            builder.emit(Inst::GetField {
                dest: old_val.clone(), obj: obj_val.clone(), field: field.clone(), ty: ty.clone(), offset,
            });
            let new_val = emit_incdec_step(builder, op, old_val.clone(), &ty);
            builder.emit(Inst::SetField { obj: obj_val, field: field.clone(), src: new_val.clone(), offset });
            if op.is_prefix() { new_val } else { old_val }
        }
        Expr::Index { object, index, .. } => {
            let obj_val = lower_expr(builder, object);
            let idx_val = lower_expr(builder, index);
            let ty = expr_ir_type_pub(builder, target);
            let (get_func, set_func) = if is_map_target(builder, object) {
                ("__map_get", "__map_set")
            } else {
                ("__array_get", "__array_set")
            };
            let old_val = builder.new_value();
            builder.emit(Inst::Call {
                dest: Some(old_val.clone()), func: get_func.into(),
                args: vec![obj_val.clone(), idx_val.clone()], ret_ty: IrType::Ptr,
            });
            let new_val = emit_incdec_step(builder, op, old_val.clone(), &ty);
            builder.emit(Inst::Call {
                dest: None, func: set_func.into(),
                args: vec![obj_val, idx_val, new_val.clone()], ret_ty: IrType::Void,
            });
            if op.is_prefix() { new_val } else { old_val }
        }
        _ => unreachable!("sema a déjà rejeté toute autre forme de cible pour Expr::IncDec"),
    }
}

/// Calcule `old_val ± 1` — `ty` est `I64` ou `F64` (jamais autre chose, sema
/// a déjà validé que la cible d'un `Expr::IncDec` est `int`/`float`).
fn emit_incdec_step(builder: &mut LowerBuilder, op: &IncDecOp, old_val: Value, ty: &IrType) -> Value {
    let one = builder.new_value();
    match ty {
        IrType::F64 => builder.emit(Inst::ConstFloat { dest: one.clone(), value: 1.0 }),
        _           => builder.emit(Inst::ConstInt   { dest: one.clone(), value: 1   }),
    }
    let new_val = builder.new_value();
    let inst = if op.is_increment() {
        Inst::Add { dest: new_val.clone(), lhs: old_val, rhs: one, ty: ty.clone() }
    } else {
        Inst::Sub { dest: new_val.clone(), lhs: old_val, rhs: one, ty: ty.clone() }
    };
    builder.emit(inst);
    new_val
}
