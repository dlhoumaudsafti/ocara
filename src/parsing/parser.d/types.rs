/// Types de base du parser

use crate::parsing::ast::Expr;
use crate::parsing::token::{Span, Token};

// ─────────────────────────────────────────────────────────────────────────────
// Erreur de parsing
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span:    Span,
}

impl ParseError {
    pub(super) fn new(msg: impl Into<String>, span: Span) -> Self {
        Self { message: msg.into(), span }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.span, self.message)
    }
}

impl std::error::Error for ParseError {}

pub type ParseResult<T> = Result<T, ParseError>;

// ─────────────────────────────────────────────────────────────────────────────
// Parser
// ─────────────────────────────────────────────────────────────────────────────

pub struct Parser {
    pub(super) tokens: Vec<Token>,
    pub(super) pos:    usize,
    /// Initialiseurs `property nom:T = expr` du corps en cours de lecture —
    /// voir `property_init.rs`.
    pub(super) property_initializers: Vec<(String, Expr, Span)>,
    /// Noms de type lus (annotations, `extends`, `implements`, `modules`,
    /// filtres `is`), avec leur position — voir `Program::type_refs`.
    pub(super) type_refs: Vec<(String, Span)>,
    /// Mode tolérant (`parse_program_recovering`) et erreurs déjà notées.
    pub(super) recover: bool,
    pub(super) errors: Vec<ParseError>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, property_initializers: Vec::new(), type_refs: Vec::new(), recover: false, errors: Vec::new() }
    }
}
