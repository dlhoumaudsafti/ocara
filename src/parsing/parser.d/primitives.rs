/// Primitives du parser

use crate::parsing::token::{Span, Token, TokenKind};
use super::types::{Parser, ParseError, ParseResult};

impl Parser {
    pub(super) fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    pub(super) fn peek_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    pub(super) fn peek_ahead(&self, offset: usize) -> Option<&Token> {
        let target_pos = self.pos + offset;
        if target_pos < self.tokens.len() {
            Some(&self.tokens[target_pos])
        } else {
            None
        }
    }

    pub(super) fn span(&self) -> Span {
        self.current().span.clone()
    }

    pub(super) fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if tok.kind != TokenKind::Eof {
            self.pos += 1;
        }
        tok
    }

    pub(super) fn check_exact(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == kind
    }

    pub(super) fn eat(&mut self, kind: &TokenKind) -> ParseResult<Token> {
        if self.check_exact(kind) {
            Ok(self.advance().clone())
        } else {
            Err(ParseError::new(
                format!("expected {:?}, found {:?}", kind, self.peek_kind()),
                self.span(),
            ))
        }
    }

    pub(super) fn eat_ident(&mut self) -> ParseResult<(String, Span)> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok((name, span))
            }
            // Permettre certains mots-clés comme identifiants dans certains contextes
            TokenKind::Init => {
                self.advance();
                Ok(("init".to_string(), span))
            }
            TokenKind::Main => {
                self.advance();
                Ok(("main".to_string(), span))
            }
            TokenKind::Error => {
                self.advance();
                Ok(("error".to_string(), span))
            }
            TokenKind::Success => {
                self.advance();
                Ok(("success".to_string(), span))
            }
            TokenKind::Exit => {
                self.advance();
                Ok(("exit".to_string(), span))
            }
            other => Err(ParseError::new(
                format!("expected identifier, found {:?}", other),
                span,
            )),
        }
    }
}
