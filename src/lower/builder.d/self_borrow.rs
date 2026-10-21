//! `self` emprunté sans retenue : une méthode est appelée avec un receveur
//! que l'appelant atteint par un chemin de références qu'il tient. Seul un
//! appel (fonction, méthode, constructeur, builtin), l'attente d'une tâche
//! ou une écriture dans un champ d'un AUTRE objet ou dans un conteneur peut
//! couper ce chemin et libérer `self` pendant la méthode ; écrire dans les
//! champs de `self` lui-même ne le peut pas. Un corps qui n'en contient
//! aucun se passe de la retenue/relâche de `self` (accesseurs, `init`
//! simples) — voir docs/roadmap.d/perf-runtime-complexite.md.

use crate::parsing::ast::{Block, Expr, Stmt, TemplatePartExpr};

/// Vrai si rien dans `body` ne peut libérer `self`.
pub fn self_stays_alive(body: &Block) -> bool {
    body.stmts.iter().all(stmt_safe)
}

fn stmt_safe(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value) | Stmt::Raise { value, .. } => expr_safe(value),
        Stmt::Return { value, .. } | Stmt::Result { value, .. } => value.as_ref().is_none_or(expr_safe),
        Stmt::Break { .. } | Stmt::Continue { .. } => true,
        Stmt::If { condition, then_block, elseif, else_block, .. } =>
            expr_safe(condition) && self_stays_alive(then_block)
                && elseif.iter().all(|(c, b)| expr_safe(c) && self_stays_alive(b))
                && else_block.as_ref().is_none_or(self_stays_alive),
        Stmt::While { condition, body, .. } => expr_safe(condition) && self_stays_alive(body),
        Stmt::ForIn { iter, body, .. } | Stmt::ForMap { iter, body, .. } => expr_safe(iter) && self_stays_alive(body),
        Stmt::Switch { subject, cases, default, .. } =>
            expr_safe(subject) && cases.iter().all(|c| self_stays_alive(&c.body)) && default.as_ref().is_none_or(self_stays_alive),
        Stmt::Try { body, handlers, .. } => self_stays_alive(body) && handlers.iter().all(|h| self_stays_alive(&h.body)),
        Stmt::Assign { target, value, .. } => expr_safe(value) && match target {
            Expr::Ident(..) => true,
            Expr::Field { object, .. } => matches!(object.as_ref(), Expr::SelfExpr(_)),
            _ => false,
        },
        _ => false,
    }
}

fn expr_safe(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_)
        | Expr::StaticConst { .. } | Expr::Nameless { .. } => true,
        Expr::Field { object, .. } => expr_safe(object),
        Expr::Binary { left, right, .. } => expr_safe(left) && expr_safe(right),
        Expr::Unary { operand, .. } => expr_safe(operand),
        Expr::IsCheck { expr, .. } => expr_safe(expr),
        Expr::IncDec { target, .. } => expr_safe(target),
        Expr::Index { object, index, .. } => expr_safe(object) && expr_safe(index),
        Expr::Range { start, end, .. } => expr_safe(start) && expr_safe(end),
        Expr::Array { elements, .. } => elements.iter().all(expr_safe),
        Expr::Map { entries, .. } => entries.iter().all(|(k, v)| expr_safe(k) && expr_safe(v)),
        Expr::Template { parts, .. } => parts.iter().all(|p| match p {
            TemplatePartExpr::Expr(e) => expr_safe(e),
            _ => true,
        }),
        Expr::Match { subject, arms, .. } => expr_safe(subject) && arms.iter().all(|a| expr_safe(&a.body)),
        _ => false,
    }
}
