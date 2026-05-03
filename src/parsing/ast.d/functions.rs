/// Déclarations de fonctions

use super::params::Param;
use super::types::Type;
use super::statements::Block;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Déclaration de fonction de niveau module
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct FuncDecl {
    pub name:     String,
    pub params:   Vec<Param>,
    pub ret_ty:   Type,
    pub body:     Block,
    pub is_async: bool,
    pub span:     Span,
}
