/// Déclarations d'énumérations

use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Variante d'un enum
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name:  String,
    /// Valeur explicite, ou None → valeur auto (index)
    pub value: Option<i64>,
    pub span:  Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Déclaration d'un enum
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name:     String,
    pub variants: Vec<EnumVariant>,
    pub span:     Span,
}
