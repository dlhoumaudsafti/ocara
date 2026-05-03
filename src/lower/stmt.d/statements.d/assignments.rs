/// Lowering des affectations

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::ir::inst::Inst;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::{lower_expr, expr_ir_type_pub};
use super::helpers::box_for_any;

pub fn lower_assign(
    builder: &mut LowerBuilder,
    target: &Expr,
    value: &Expr,
) {
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
