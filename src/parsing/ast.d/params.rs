/// Paramètres de fonctions et méthodes

use super::types::Type;
use super::expressions::Expr;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Paramètre de fonction / constructeur
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name:          String,
    pub ty:            Type,
    pub default_value: Option<Expr>,
    pub is_variadic:   bool,
    pub span:          Span,
}
