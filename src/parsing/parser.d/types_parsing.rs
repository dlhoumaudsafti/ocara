/// Parsing des types

use crate::parsing::ast::Type;
use crate::parsing::token::TokenKind;
use super::types::{Parser, ParseError, ParseResult};

impl Parser {
    pub(super) fn parse_type(&mut self) -> ParseResult<Type> {
        let first = self.parse_type_base()?;

        // Type union : `T | U | ...`
        if self.check_exact(&TokenKind::Pipe) {
            let mut variants = vec![first];
            while self.check_exact(&TokenKind::Pipe) {
                self.advance();
                variants.push(self.parse_type_base()?);
            }
            return Ok(Type::Union(variants));
        }

        Ok(first)
    }

    /// Analyse un type de base sans consommer les `|` qui suivent (appelé depuis parse_type pour les unions).
    fn parse_type_base(&mut self) -> ParseResult<Type> {
        let base = match self.peek_kind().clone() {
            TokenKind::TInt    => { self.advance(); Type::Int    }
            TokenKind::TFloat  => { self.advance(); Type::Float  }
            TokenKind::TString => { self.advance(); Type::String }
            TokenKind::TBool   => { self.advance(); Type::Bool   }
            TokenKind::TMixed    => { self.advance(); Type::Mixed    }
            TokenKind::TVoid   => { self.advance(); Type::Void   }
            TokenKind::LitNull => { self.advance(); Type::Null   }

            TokenKind::TMap => {
                self.advance();
                self.eat(&TokenKind::Lt)?;
                let k = self.parse_type()?;
                self.eat(&TokenKind::Comma)?;
                let v = self.parse_type()?;
                self.eat(&TokenKind::Gt)?;
                Type::Map(Box::new(k), Box::new(v))
            }

            TokenKind::Ident(name) => {
                self.advance();
                // `Function<ReturnType(ParamType, ...)>`
                if name == "Function" {
                    self.eat(&TokenKind::Lt)?;
                    let ret_ty = self.parse_type()?;
                    
                    // Les paramètres sont obligatoires
                    self.eat(&TokenKind::LParen)?;
                    let mut param_tys = Vec::new();
                    if !self.check_exact(&TokenKind::RParen) {
                        param_tys.push(self.parse_type()?);
                        while self.check_exact(&TokenKind::Comma) {
                            self.advance();
                            param_tys.push(self.parse_type()?);
                        }
                    }
                    self.eat(&TokenKind::RParen)?;
                    
                    self.eat(&TokenKind::Gt)?;
                    return Ok(Type::Function {
                        ret_ty: Box::new(ret_ty),
                        param_tys,
                    });
                }
                
                // Type qualifié : repository.User
                if self.check_exact(&TokenKind::Dot) {
                    let mut parts = vec![name];
                    while self.check_exact(&TokenKind::Dot) {
                        self.advance();
                        parts.push(self.eat_ident()?.0);
                    }
                    Type::Qualified(parts)
                } 
                // Type générique : List<int>, Cache<string, User>
                else if self.check_exact(&TokenKind::Lt) {
                    self.advance(); // '<'
                    let mut args = Vec::new();
                    args.push(self.parse_type()?);
                    while self.check_exact(&TokenKind::Comma) {
                        self.advance();
                        args.push(self.parse_type()?);
                    }
                    self.eat(&TokenKind::Gt)?; // '>'
                    Type::Generic { name, args }
                } 
                // Type nommé simple
                else {
                    Type::Named(name)
                }
            }

            other => {
                return Err(ParseError::new(
                    format!("expected type, found {:?}", other),
                    self.span(),
                ))
            }
        };

        let mut ty = base;
        while self.check_exact(&TokenKind::LBracket) {
            self.advance();
            self.eat(&TokenKind::RBracket)?;
            ty = Type::Array(Box::new(ty));
        }
        Ok(ty)
    }
}
