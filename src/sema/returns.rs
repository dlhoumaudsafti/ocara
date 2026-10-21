/// Analyse « tous les chemins retournent » d'un corps de fonction non-`void`
/// (E67) : sans elle, `function g(x:int): string { if x greater 0 { return
/// "pos" } }` rendait `null` (ou `0`) sur le chemin oublié.

use crate::parsing::ast::{Block, Expr, Literal, Stmt};

/// Le bloc ne se termine jamais normalement : chaque chemin finit par
/// `return` ou `raise` (ou une boucle `while true` sans `break`).
pub fn always_returns(block: &Block) -> bool {
    block.stmts.iter().any(stmt_returns)
}

fn stmt_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return { .. } | Stmt::Raise { .. } => true,
        Stmt::If { then_block, elseif, else_block: Some(else_block), .. } =>
            always_returns(then_block) && elseif.iter().all(|(_, b)| always_returns(b)) && always_returns(else_block),
        Stmt::Switch { cases, default: Some(default), .. } =>
            cases.iter().all(|c| always_returns(&c.body)) && always_returns(default),
        Stmt::Try { body, handlers, .. } =>
            always_returns(body) && handlers.iter().all(|h| always_returns(&h.body)),
        Stmt::While { condition: Expr::Literal(Literal::Bool(true), _), body, .. } => !breaks(body),
        _ => false,
    }
}

/// Un `break` qui sort de la boucle courante (pas d'une boucle imbriquée).
fn breaks(block: &Block) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        Stmt::Break { .. } => true,
        Stmt::If { then_block, elseif, else_block, .. } =>
            breaks(then_block) || elseif.iter().any(|(_, b)| breaks(b)) || else_block.as_ref().is_some_and(breaks),
        Stmt::Switch { cases, default, .. } =>
            cases.iter().any(|c| breaks(&c.body)) || default.as_ref().is_some_and(breaks),
        Stmt::Try { body, handlers, .. } => breaks(body) || handlers.iter().any(|h| breaks(&h.body)),
        _ => false,
    })
}
