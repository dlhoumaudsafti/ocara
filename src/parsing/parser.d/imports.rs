/// Parsing des imports et constantes globales

use crate::parsing::ast::{ImportDecl, ConstDecl};
use crate::parsing::token::TokenKind;
use super::types::{Parser, ParseError, ParseResult};

impl Parser {
    pub(super) fn parse_import(&mut self) -> ParseResult<ImportDecl> {
        let span = self.span();
        self.eat(&TokenKind::Import)?;

        // Deux formats possibles:
        // 1. Ancien: import ocara.IO [as Alias]
        // 2. Nouveau: import Circle from "file" [as Alias]
        //            import * from "file"
        
        // Premier élément: soit un identifiant, soit *
        let mut path = vec![];
        if self.check_exact(&TokenKind::Star) {
            self.advance();
            path.push("*".to_string());
        } else {
            path.push(self.eat_ident()?.0);
        }

        // Vérifier si c'est le format "from"
        if self.check_exact(&TokenKind::From) {
            self.advance(); // 'from'
            
            // Attendre une string littérale pour le chemin de fichier
            let file_path = match &self.current().kind {
                TokenKind::LitString(s) => {
                    let path = s.clone();
                    self.advance();
                    Some(path)
                }
                _ => {
                    return Err(ParseError {
                        message: "expected string literal after 'from'".to_string(),
                        span: self.span(),
                    });
                }
            };

            // Alias optionnel
            let alias = if self.check_exact(&TokenKind::As) {
                self.advance();
                Some(self.eat_ident()?.0)
            } else {
                None
            };

            return Ok(ImportDecl { path, file_path, alias, span });
        }

        // Sinon, format ancien: continuer avec les points
        while self.check_exact(&TokenKind::Dot) {
            self.advance(); // '.'
            // Accepter `*` (Star) comme dernier segment : import ocara.*
            if self.check_exact(&TokenKind::Star) {
                self.advance();
                path.push("*".to_string());
                break;
            }
            path.push(self.eat_ident()?.0);
        }

        let alias = if self.check_exact(&TokenKind::As) {
            self.advance();
            Some(self.eat_ident()?.0)
        } else {
            None
        };

        Ok(ImportDecl { path, file_path: None, alias, span })
    }

    pub(super) fn parse_const_decl(&mut self) -> ParseResult<ConstDecl> {
        let span = self.span();
        self.eat(&TokenKind::Const)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.eat(&TokenKind::Eq)?;
        let value = self.parse_expr()?;
        Ok(ConstDecl { name, ty, value, span })
    }
}
