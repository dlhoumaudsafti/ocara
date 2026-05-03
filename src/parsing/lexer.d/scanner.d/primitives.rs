/// Fonctions primitives de navigation dans le source

use super::super::types::Lexer;
use crate::parsing::token::Span;

impl Lexer {
    /// Caractère courant (sans avancer).
    pub(in crate::parsing::lexer) fn current(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    /// Caractère suivant (look-ahead 1, sans avancer).
    pub(in crate::parsing::lexer) fn peek_next(&self) -> Option<char> {
        self.source.get(self.pos + 1).copied()
    }

    /// Avance d'un caractère et met à jour ligne/colonne.
    pub(in crate::parsing::lexer) fn advance(&mut self) -> Option<char> {
        let ch = self.source.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    /// Retourne le `Span` du prochain caractère à lire.
    pub(in crate::parsing::lexer) fn span(&self) -> Span {
        Span::new(self.line, self.col)
    }
}
