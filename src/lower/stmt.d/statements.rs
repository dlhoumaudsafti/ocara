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
        Stmt::Var { name, ty, value, mutable, kind, .. } => {
            lower_var(builder, name, ty, value, *mutable, *kind);
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
            // `return x` : `x` peut être une `scoped`/`consumed` qui
            // s'échappe hors de son bloc (voir crate::lower::stmt::ownership).
            let v = value.as_ref().map(|e| {
                let val = lower_expr(builder, e);
                crate::lower::stmt::ownership::maybe_clone_escaping(builder, e, val)
            });
            // Sortie anticipée de la fonction : détruit toutes les
            // scoped/consumed encore vivantes dans les blocs actuellement
            // ouverts (v, calculé juste au-dessus, a déjà sa propre copie
            // indépendante si besoin — voir maybe_clone_escaping).
            crate::lower::stmt::ownership::emit_early_exit_drops(builder, 0);
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
            } else {
                // Fonction normale : vrai return.
                // (`return` est rejeté par la sema à l'intérieur d'un bloc runtime —
                // voir Stmt::Result pour le sucre ERROR/jump propre aux blocs runtime.)
                builder.emit(Inst::Return { value: v });
            }
        }

        // `result expr` — uniquement valide (sémantiquement) dans un bloc runtime :
        // fixe ERROR puis saute vers le label de sortie anticipée (runtime_exit_bb)
        // sans quitter la fonction main() synthétisée. Remplace l'ancien sucre `return`.
        Stmt::Result { value, .. } => {
            // Cas particulier : `result` à l'intérieur d'un handler d'exception
            // (try/on), y compris quand ce handler est imbriqué dans un bloc
            // runtime (ex: `init { try { ... } on e { result e.code } }`).
            // Un handler est lowered comme une fonction séparée (__try_handler_*),
            // donc on ne peut pas sauter vers runtime_exit_bb depuis là : on
            // reprend le même mécanisme de propagation que `return` dans un
            // handler (comportement historique, préservé à l'identique).
            if builder.func.name.starts_with("__try_handler_") {
                let v = value.as_ref().map(|e| lower_expr(builder, e));
                let return_val = v.clone().unwrap_or_else(|| {
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
            } else if builder.func.name == "main" && builder.runtime_exit_bb.is_some() {
                // Gérer le cas spécial de "result SUCCESS" : SUCCESS est bool mais ERROR est int
                // Convertir SUCCESS (bool) en 0 (int) directement (sans lowering, calculé
                // une seule fois pour éviter de dupliquer les instructions émises).
                let is_success_ident = matches!(value.as_ref(), Some(Expr::Ident(name, _)) if name == "SUCCESS");
                let result_val = if is_success_ident || value.is_none() {
                    // result SUCCESS, ou result (sans valeur) → ERROR = 0
                    let zero = builder.new_value();
                    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                    zero
                } else {
                    lower_expr(builder, value.as_ref().unwrap())
                };

                // ERROR = result_value
                if let Some((error_slot, _, _)) = builder.locals.get("ERROR").cloned() {
                    builder.emit(Inst::Store { ptr: error_slot.clone(), src: result_val.clone() });

                    // Sauter au label de sortie anticipée du bloc main
                    if let Some(exit_bb) = builder.runtime_exit_bb.clone() {
                        builder.emit(Inst::Jump { target: exit_bb });
                        return; // Ne pas émettre de Return après
                    }
                }

                // Fallback : émettre return normal si pas de runtime_exit_bb
                builder.emit(Inst::Return { value: Some(result_val) });
            } else {
                // Fallback défensif (error/success/exit, ou contexte inattendu) :
                // se comporte comme un vrai return de la fonction main() synthétisée.
                let v = value.as_ref().map(|e| lower_expr(builder, e));
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
