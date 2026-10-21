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
    /// Lecture d'une clé de littéral map : le `:` qui suit `obj.champ` sépare
    /// la clé de la valeur, ce n'est pas une annotation de type.
    pub(super) in_map_key: bool,
    /// `wiring <chemin>` lus dans le corps de classe en cours — repris par
    /// `parse_class` (classe de test, voir `core::test_wiring`).
    pub(super) class_wirings: Vec<crate::parsing::ast::WiringDecl>,
    /// Profondeur de l'arbre de l'expression en cours (imbrication et
    /// opérateurs enchaînés) — bornée par `MAX_EXPR_DEPTH`, voir
    /// `expressions.rs`.
    pub(super) expr_depth: usize,
    /// Profondeur d'imbrication des blocs en cours — bornée par
    /// `MAX_BLOCK_DEPTH`, voir `statements.rs`.
    pub(super) block_depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, property_initializers: Vec::new(), type_refs: Vec::new(), recover: false, errors: Vec::new(), in_map_key: false, class_wirings: Vec::new(), expr_depth: 0, block_depth: 0 }
    }
}
