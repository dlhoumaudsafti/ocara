/// Parsing des blocs runtime

use crate::parsing::ast::{RuntimeImport, RuntimeBlock, RuntimeBlockKind};
use crate::parsing::token::TokenKind;
use super::types::{Parser, ParseError, ParseResult};

impl Parser {
    /// Parse un import runtime : `runtime start.Init as init`
    pub(super) fn parse_runtime_import(&mut self) -> ParseResult<RuntimeImport> {
        let span = self.span();
        self.eat(&TokenKind::Runtime)?;

        // Parser le chemin : logger ou start.Init
        let mut path = vec![self.eat_ident()?.0];
        while self.check_exact(&TokenKind::Dot) {
            self.advance();
            path.push(self.eat_ident()?.0);
        }

        // `is <block_kind>` est optionnel
        let kind = if self.check_exact(&TokenKind::Is) {
            self.advance();
            let kind_name = self.eat_ident()?.0;
            
            // Convertir le nom en RuntimeBlockKind
            let k = match kind_name.as_str() {
                "init" => RuntimeBlockKind::Init,
                "main" => RuntimeBlockKind::Main,
                "error" => RuntimeBlockKind::Error,
                "success" => RuntimeBlockKind::Success,
                "exit" => RuntimeBlockKind::Exit,
                other => {
                    return Err(ParseError::new(
                        format!("invalid runtime block kind '{}', expected init, main, error, success, or exit", other),
                        span,
                    ))
                }
            };
            Some(k)
        } else {
            None
        };

        Ok(RuntimeImport { path, kind, span })
    }

    /// Parse un bloc runtime : init { ... }, main { ... }, etc.
    pub(super) fn parse_runtime_block(&mut self) -> ParseResult<RuntimeBlock> {
        let span = self.span();
        
        // Déterminer le type de bloc
        let kind = match self.peek_kind() {
            TokenKind::Init => RuntimeBlockKind::Init,
            TokenKind::Main => RuntimeBlockKind::Main,
            TokenKind::Error => RuntimeBlockKind::Error,
            TokenKind::Success => RuntimeBlockKind::Success,
            TokenKind::Exit => RuntimeBlockKind::Exit,
            other => {
                return Err(ParseError::new(
                    format!("expected runtime block keyword, found {:?}", other),
                    span,
                ))
            }
        };
        self.advance(); // consommer le mot-clé

        // Parser le corps du bloc : { statements }
        self.eat(&TokenKind::LBrace)?;
        let mut statements = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.check_exact(&TokenKind::Eof) {
            statements.push(self.parse_stmt()?);
        }
        self.eat(&TokenKind::RBrace)?;

        Ok(RuntimeBlock { kind, statements, span })
    }
}
