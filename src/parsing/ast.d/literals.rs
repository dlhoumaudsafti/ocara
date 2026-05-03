/// Littéraux et fragments de templates

use super::expressions::Expr;

// ─────────────────────────────────────────────────────────────────────────────
// Littéraux
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

// ─────────────────────────────────────────────────────────────────────────────
// TemplatePartExpr — fragment AST d'une chaîne template
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePartExpr {
    Literal(String),
    Expr(Box<Expr>),
}
