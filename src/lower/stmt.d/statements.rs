/// Dispatcher pour le lowering des instructions

#[path = "statements.d/mod.rs"]
mod statements_impl;

use crate::parsing::ast::*;
use crate::ir::inst::Inst;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::lower_expr;
use statements_impl::*;

pub fn lower_stmt(builder: &mut LowerBuilder, stmt: &Stmt) {
    match stmt {
        // ── Déclarations ──────────────────────────────────────────────────────
        Stmt::Var { name, ty, value, mutable, .. } => {
            lower_var(builder, name, ty, value, *mutable);
        }
        Stmt::Const { name, ty, value, .. } => {
            lower_const(builder, name, ty, value);
        }

        // ── Contrôle de flux ──────────────────────────────────────────────────
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            lower_if(builder, condition, then_block, elseif, else_block);
        }
        Stmt::Switch { subject, cases, default, .. } => {
            lower_switch(builder, subject, cases, default);
        }
        Stmt::While { condition, body, .. } => {
            lower_while(builder, condition, body);
        }

        // ── Boucles ───────────────────────────────────────────────────────────
        Stmt::ForIn { var, iter, body, .. } => {
            lower_for_in(builder, var, iter, body);
        }
        Stmt::ForMap { key, value, iter, body, .. } => {
            lower_for_map(builder, key, value, iter, body);
        }
        Stmt::Break { .. } => {
            lower_break(builder);
        }
        Stmt::Continue { .. } => {
            lower_continue(builder);
        }

        // ── Affectation ───────────────────────────────────────────────────────
        Stmt::Assign { target, value, .. } => {
            lower_assign(builder, target, value);
        }

        // ── Simples (inline) ──────────────────────────────────────────────────
        Stmt::Expr(expr) => {
            lower_expr(builder, expr);
        }
        Stmt::Return { value, .. } => {
            let v = value.as_ref().map(|e| lower_expr(builder, e));
            builder.emit(Inst::Return { value: v });
        }

        // ── Exceptions ────────────────────────────────────────────────────────
        Stmt::Raise { value, .. } => {
            lower_raise(builder, value);
        }
        Stmt::Try { body, handlers, .. } => {
            lower_try(builder, body, handlers);
        }
    }
}
