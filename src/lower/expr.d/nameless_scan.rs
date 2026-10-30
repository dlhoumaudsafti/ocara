/// Présence d'une `nameless` ou d'un `try` dans un bloc — tous deux
/// compilés en fonctions séparées qui capturent des variables — mémorisée
/// par bloc : la
/// pré-promotion des captures (`hoist_closure_promotions`), appelée à chaque
/// `if`/boucle, reparcourait tout le sous-arbre à chaque niveau —
/// quadratique sur des instructions imbriquées
/// (docs/roadmap.d/compilateur-limites-echelle-residuelles.md).

use std::collections::HashMap;

use crate::parsing::ast::{Block, Expr, Stmt, TemplatePartExpr};

/// Résultat par bloc (adresse du bloc, l'AST restant en place pendant le
/// lowering).
pub type NamelessMemo = HashMap<*const Block, bool>;

pub fn block_has_nameless(block: &Block, memo: &mut NamelessMemo) -> bool {
    let key = block as *const Block;
    if let Some(&known) = memo.get(&key) {
        return known;
    }
    let found = block.stmts.iter().any(|s| stmt_has_nameless(s, memo));
    memo.insert(key, found);
    found
}

fn stmt_has_nameless(stmt: &Stmt, memo: &mut NamelessMemo) -> bool {
    match stmt {
        Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value)
        | Stmt::Raise { value, .. } | Stmt::Emit { value, .. } => expr_has_nameless(value),
        Stmt::Assign { target, value, .. } => expr_has_nameless(target) || expr_has_nameless(value),
        Stmt::Return { value, .. } | Stmt::Result { value, .. } => value.as_ref().is_some_and(expr_has_nameless),
        Stmt::Break { .. } | Stmt::Continue { .. } => false,
        Stmt::If { condition, then_block, elseif, else_block, .. } =>
            expr_has_nameless(condition) || block_has_nameless(then_block, memo)
                || elseif.iter().any(|(c, b)| expr_has_nameless(c) || block_has_nameless(b, memo))
                || else_block.as_ref().is_some_and(|b| block_has_nameless(b, memo)),
        Stmt::While { condition, body, .. } => expr_has_nameless(condition) || block_has_nameless(body, memo),
        Stmt::ForIn { iter, body, .. } | Stmt::ForMap { iter, body, .. } => expr_has_nameless(iter) || block_has_nameless(body, memo),
        Stmt::Switch { subject, cases, default, .. } =>
            expr_has_nameless(subject) || cases.iter().any(|c| block_has_nameless(&c.body, memo))
                || default.as_ref().is_some_and(|b| block_has_nameless(b, memo)),
        // Corps et gestionnaires d'un `try` : fonctions séparées, comme une
        // `nameless` (leurs captures doivent être promues avant une boucle).
        Stmt::Try { .. } => true,
    }
}

pub fn expr_has_nameless(expr: &Expr) -> bool {
    match expr {
        Expr::Nameless { .. } => true,
        Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => false,
        Expr::Field { object, .. } => expr_has_nameless(object),
        Expr::Call { callee, args, .. } => expr_has_nameless(callee) || args.iter().any(expr_has_nameless),
        Expr::StaticCall { args, .. } | Expr::New { args, .. } => args.iter().any(expr_has_nameless),
        Expr::Binary { left, right, .. } => expr_has_nameless(left) || expr_has_nameless(right),
        Expr::Unary { operand, .. } => expr_has_nameless(operand),
        Expr::Array { elements, .. } => elements.iter().any(expr_has_nameless),
        Expr::Map { entries, .. } => entries.iter().any(|(k, v)| expr_has_nameless(k) || expr_has_nameless(v)),
        Expr::Template { parts, .. } => parts.iter().any(|p| matches!(p, TemplatePartExpr::Expr(e) if expr_has_nameless(e))),
        Expr::Index { object, index, .. } => expr_has_nameless(object) || expr_has_nameless(index),
        Expr::Range { start, end, .. } => expr_has_nameless(start) || expr_has_nameless(end),
        Expr::Match { subject, arms, .. } => expr_has_nameless(subject) || arms.iter().any(|a| expr_has_nameless(&a.body)),
        Expr::Cond { cond, then_expr, else_expr, .. } =>
            expr_has_nameless(cond) || expr_has_nameless(then_expr) || else_expr.value().is_some_and(expr_has_nameless),
        Expr::Resolve { expr, .. } | Expr::IsCheck { expr, .. } => expr_has_nameless(expr),
        Expr::IncDec { target, .. } => expr_has_nameless(target),
        Expr::NamedArg { value, .. } => expr_has_nameless(value),
    }
}
