/// Déclarations d'interfaces

use super::types::Type;
use super::params::Param;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Méthode d'interface (signature seule)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceMethod {
    pub name:   String,
    pub params: Vec<Param>,
    pub ret_ty: Type,
    pub span:   Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Déclaration d'interface
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    pub name:    String,
    pub methods: Vec<InterfaceMethod>,
    pub span:    Span,
}
