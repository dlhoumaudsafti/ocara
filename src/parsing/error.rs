use std::fmt;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// LexError  –  erreurs produites par le lexer
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    /// Caractère non reconnu par la grammaire Ocara.
    UnexpectedChar(char, Span),

    /// Chaîne de caractères ouverte sans guillemet fermant.
    UnterminatedString(Span),

    /// Séquence d'échappement inconnue dans une chaîne.
    InvalidEscape(char, Span),

    /// Entier trop grand pour un i64.
    IntegerOverflow(String, Span),

    /// Nombre en notation à exposant (`1.0e300`), non supportée.
    ExponentNotation(String, Span),
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexError::UnexpectedChar(ch, span) =>
                write!(f, "[{}] Caractère inattendu '{}'", span, ch),

            LexError::UnterminatedString(span) =>
                write!(f, "[{}] Chaîne non fermée", span),

            LexError::InvalidEscape(ch, span) =>
                write!(f, "[{}] Séquence d'échappement invalide '\\{}'", span, ch),

            LexError::IntegerOverflow(raw, span) =>
                write!(f, "[{}] Entier trop grand : {}", span, raw),

            LexError::ExponentNotation(raw, span) =>
                write!(f, "[{}] Notation à exposant non supportée : {}", span, raw),
        }
    }
}

impl std::error::Error for LexError {}
