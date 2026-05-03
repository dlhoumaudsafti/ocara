/// Types de base du parser

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
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }
}
