/// Parsing du programme principal

use crate::parsing::ast::Program;
use crate::parsing::token::TokenKind;
use super::types::{Parser, ParseError, ParseResult};

impl Parser {
    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let mut program = Program::new();

        // Parser le namespace optionnel en début de fichier
        if self.check_exact(&TokenKind::Namespace) {
            self.advance(); // consommer 'namespace'
            
            // namespace . ou namespace identifier[.identifier]*
            if self.check_exact(&TokenKind::Dot) {
                self.advance(); // consommer '.'
                program.namespace = Some(".".to_string()); // namespace racine explicite
            } else {
                // Doit être un identifiant (ou plusieurs séparés par des points)
                let tok = self.current().clone();
                if let TokenKind::Ident(_) = tok.kind {
                    let mut ns_parts = vec![tok.lexeme.clone()];
                    self.advance();
                    
                    // Lire les parties supplémentaires du namespace (ex: configs.components)
                    while self.check_exact(&TokenKind::Dot) {
                        self.advance(); // consommer '.'
                        let tok = self.current().clone();
                        if let TokenKind::Ident(_) = tok.kind {
                            ns_parts.push(tok.lexeme.clone());
                            self.advance();
                        } else {
                            return Err(ParseError::new(
                                "expected identifier after '.' in namespace declaration".to_string(),
                                self.span(),
                            ));
                        }
                    }
                    
                    program.namespace = Some(ns_parts.join("."));
                } else {
                    return Err(ParseError::new(
                        "expected '.' or identifier after 'namespace'".to_string(),
                        self.span(),
                    ));
                }
            }
        }

        while !self.check_exact(&TokenKind::Eof) {
            match self.peek_kind().clone() {
                TokenKind::Import => {
                    program.imports.push(self.parse_import()?);
                }
                TokenKind::Runtime => {
                    program.runtime_imports.push(self.parse_runtime_import()?);
                }
                TokenKind::Init | TokenKind::Main | TokenKind::Error 
                | TokenKind::Success | TokenKind::Exit => {
                    program.runtime_blocks.push(self.parse_runtime_block()?);
                }
                TokenKind::Const => {
                    program.consts.push(self.parse_const_decl()?);
                }
                TokenKind::Module => {
                    program.modules.push(self.parse_module()?);
                }
                TokenKind::Enum => {
                    program.enums.push(self.parse_enum()?);
                }
                TokenKind::Class => {
                    program.classes.push(self.parse_class()?);
                }
                TokenKind::Generic => {
                    program.generics.push(self.parse_generic()?);
                }
                TokenKind::Interface => {
                    program.interfaces.push(self.parse_interface()?);
                }
                TokenKind::Function => {
                    program.functions.push(self.parse_func()?);
                }
                TokenKind::Async => {
                    // `async function ...` au niveau du module
                    program.functions.push(self.parse_func()?);
                }
                other => {
                    return Err(ParseError::new(
                        format!("unexpected top-level declaration: {:?}", other),
                        self.span(),
                    ))
                }
            }
        }

        Ok(program)
    }
}
