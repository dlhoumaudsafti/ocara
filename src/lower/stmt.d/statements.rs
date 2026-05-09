/// Dispatcher pour le lowering des instructions

#[path = "statements.d/mod.rs"]
mod statements_impl;

use crate::parsing::ast::*;
use crate::ir::inst::Inst;
use crate::ir::types::IrType;
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
            
            // Si on est dans un handler d'exception (__try_handler_*), signaler le return
            // au runtime pour qu'il soit propagé à la fonction englobante
            if builder.func.name.starts_with("__try_handler_") {
                let return_val = v.clone().unwrap_or_else(|| {
                    // Return void → passer 0
                    let zero = builder.new_value();
                    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                    zero
                });
                
                builder.emit(Inst::Call {
                    dest: None,
                    func: "__ocara_handler_set_return".into(),
                    args: vec![return_val],
                    ret_ty: IrType::Void,
                });
                
                builder.emit(Inst::Return { value: v });
            }
            // Si on est dans la fonction main (bloc runtime), transformer le return
            // en assignation ERROR puis sauter vers runtime_exit_bb
            else if builder.func.name == "main" && builder.runtime_exit_bb.is_some() {
                // Gérer le cas spécial de "return SUCCESS" : SUCCESS est bool mais ERROR est int
                // Convertir SUCCESS (bool) en 0 (int) directement
                let return_val = if let Some(expr) = value.as_ref() {
                    if let Expr::Ident(name, _) = expr {
                        if name == "SUCCESS" {
                            // return SUCCESS → ERROR = 0
                            let zero = builder.new_value();
                            builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                            zero
                        } else {
                            lower_expr(builder, expr)
                        }
                    } else {
                        lower_expr(builder, expr)
                    }
                } else {
                    // Return void → ERROR = 0
                    let zero = builder.new_value();
                    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                    zero
                };
                
                // ERROR = return_value
                if let Some((error_slot, _, _)) = builder.locals.get("ERROR").cloned() {
                    builder.emit(Inst::Store { ptr: error_slot.clone(), src: return_val.clone() });
                    
                    // Sauter au label de sortie anticipée du bloc main
                    if let Some(exit_bb) = builder.runtime_exit_bb.clone() {
                        builder.emit(Inst::Jump { target: exit_bb });
                        return; // Ne pas émettre de Return après
                    }
                }
                
                // Fallback : émettre return normal si pas de runtime_exit_bb
                builder.emit(Inst::Return { value: Some(return_val) });
            }
            else {
                // Fonction normale : vrai return
                builder.emit(Inst::Return { value: v });
            }
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
