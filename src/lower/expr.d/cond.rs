/// Lowering de `cond then a else b` — voir `docs/EBNF.md` §11.3.
///
/// Seule la branche choisie est évaluée. Les deux branches écrivent dans
/// un même emplacement ; si leurs types IR diffèrent, la valeur est un
/// `mixed` et chaque branche scalaire est boxée. Les valeurs par défaut de
/// la forme courte (`empty`/`zero`/`null`) sont déjà des littéraux
/// (`core::named_args`).

use crate::parsing::ast::{CondElse, Expr};
use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::lower::stmt::rc;
use super::helpers::box_scalar_arg;
use super::lower::{hoist_closure_promotions_in_exprs, lower_condition, lower_expr};
use super::typeinfer::expr_ir_type;

fn else_value(else_expr: &CondElse) -> &Expr {
    else_expr.value().expect("valeur par défaut d'un `then` non réécrite après la sema")
}

/// Type IR du résultat : celui des branches, ou `Ptr` (`mixed`) s'ils diffèrent.
pub fn result_ir_type(builder: &LowerBuilder, then_expr: &Expr, else_expr: &CondElse) -> IrType {
    branches_ir_type(builder, then_expr, else_value(else_expr))
}

fn branches_ir_type(builder: &LowerBuilder, then_expr: &Expr, else_expr: &Expr) -> IrType {
    let then_ty = expr_ir_type(builder, then_expr);
    if expr_ir_type(builder, else_expr) == then_ty { then_ty } else { IrType::Ptr }
}

pub fn lower_cond(builder: &mut LowerBuilder, cond: &Expr, then_expr: &Expr, else_expr: &CondElse) -> Value {
    let else_expr = else_value(else_expr);
    let ty = branches_ir_type(builder, then_expr, else_expr);
    // Si une branche produit une valeur possédée (ou boxée), toutes rendent
    // une valeur possédée.
    let owned = ty == IrType::Ptr && [then_expr, else_expr].iter().any(|e| {
        expr_ir_type(builder, e) != ty
            || matches!(e, Expr::Array { .. } | Expr::Map { .. })
            || rc::produces_owned(builder, e)
    });
    hoist_closure_promotions_in_exprs(builder, &[then_expr, else_expr]);
    let slot = builder.new_value();
    builder.emit(Inst::Alloca { dest: slot.clone(), ty: ty.clone() });
    let test = lower_condition(builder, cond);
    let then_bb = builder.new_block();
    let else_bb = builder.new_block();
    let merge_bb = builder.new_block();
    builder.emit(Inst::Branch { cond: test, then_bb: then_bb.clone(), else_bb: else_bb.clone() });
    for (bb, branch) in [(then_bb, then_expr), (else_bb, else_expr)] {
        builder.switch_to(&bb);
        let v = lower_branch(builder, branch, &ty, owned);
        builder.emit(Inst::Store { ptr: slot.clone(), src: v });
        if !builder.is_terminated() {
            builder.emit(Inst::Jump { target: merge_bb.clone() });
        }
    }
    builder.switch_to(&merge_bb);
    let dest = builder.new_value();
    builder.emit(Inst::Load { dest: dest.clone(), ptr: slot, ty });
    if owned {
        rc::track(builder, &dest);
    }
    dest
}

/// Branche : ses temporaires sont relâchés dans la branche (le bloc de
/// fusion ne les voit pas).
fn lower_branch(builder: &mut LowerBuilder, branch: &Expr, ty: &IrType, owned: bool) -> Value {
    rc::begin_temps(builder);
    let branch_ty = expr_ir_type(builder, branch);
    let v = lower_expr(builder, branch);
    let v = if branch_ty != *ty {
        box_scalar_arg(builder, &branch_ty, v)
    } else {
        if owned && !rc::claim(builder, &v) {
            rc::retain(builder, &v);
        }
        v
    };
    rc::end_temps(builder);
    v
}
