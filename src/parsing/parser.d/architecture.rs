//! `architecture permissive` / `architecture hexagonal [{ src: a.b,
//! layers: { infrastructure: infra, … }, shared: [a.b, …] }]` — virgule
//! finale permise partout. `architecture` reste un identifiant ordinaire
//! hors de cette position (début de déclaration de premier niveau).

use crate::parsing::ast::{ArchitectureDecl, ArchitectureMode};
use crate::parsing::token::{Span, TokenKind};
use super::types::{ParseError, ParseResult, Parser};

const LAYERS: [&str; 3] = ["infrastructure", "application", "domain"];

impl Parser {
    /// `architecture` suivi de `hexagonal` ou `permissive`.
    pub(super) fn at_architecture(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Ident(n) if n == "architecture")
            && self.peek_ahead(1).is_some_and(|t| matches!(&t.kind, TokenKind::Ident(m) if m == "hexagonal" || m == "permissive"))
    }

    pub(super) fn parse_architecture(&mut self) -> ParseResult<ArchitectureDecl> {
        let span = self.span();
        self.advance();
        let (mode, _) = self.eat_ident()?;
        let mode = if mode == "hexagonal" { ArchitectureMode::Hexagonal } else { ArchitectureMode::Permissive };
        let mut decl = ArchitectureDecl { mode, src: None, layers: Vec::new(), shared: Vec::new(), span };
        if !self.check_exact(&TokenKind::LBrace) {
            return Ok(decl);
        }
        if decl.mode == ArchitectureMode::Permissive {
            return Err(ParseError::new("'architecture permissive' n'accepte pas de configuration", self.span()));
        }
        self.advance();
        while !self.check_exact(&TokenKind::RBrace) {
            let (key, key_span) = self.eat_ident()?;
            self.eat(&TokenKind::Colon)?;
            match key.as_str() {
                "src" => decl.src = Some(self.dotted_path()?),
                "layers" => decl.layers = self.layer_aliases()?,
                "shared" => {
                    self.eat(&TokenKind::LBracket)?;
                    while !self.check_exact(&TokenKind::RBracket) {
                        decl.shared.push(self.dotted_path()?);
                        if !self.eat_comma() { break; }
                    }
                    self.eat(&TokenKind::RBracket)?;
                }
                other => return Err(ParseError::new(format!("architecture : propriété inconnue '{}' (attendu src, layers ou shared)", other), key_span)),
            }
            if !self.eat_comma() { break; }
        }
        self.eat(&TokenKind::RBrace)?;
        Ok(decl)
    }

    /// `{ infrastructure: infra, application: app, domain: dom }`.
    fn layer_aliases(&mut self) -> ParseResult<Vec<(String, String, Span)>> {
        self.eat(&TokenKind::LBrace)?;
        let mut out: Vec<(String, String, Span)> = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            let (layer, span) = self.eat_ident()?;
            if !LAYERS.contains(&layer.as_str()) {
                return Err(ParseError::new(format!("architecture : couche inconnue '{}' (attendu infrastructure, application ou domain)", layer), span));
            }
            if out.iter().any(|(l, _, _)| *l == layer) {
                return Err(ParseError::new(format!("architecture : couche '{}' déclarée deux fois", layer), span));
            }
            self.eat(&TokenKind::Colon)?;
            let (alias, _) = self.eat_ident()?;
            out.push((layer, alias, span));
            if !self.eat_comma() { break; }
        }
        self.eat(&TokenKind::RBrace)?;
        Ok(out)
    }

    /// `a.b.c` → ("a.b.c", position).
    fn dotted_path(&mut self) -> ParseResult<(String, Span)> {
        let (first, span) = self.eat_ident()?;
        let mut parts = vec![first];
        while self.check_exact(&TokenKind::Dot) {
            self.advance();
            parts.push(self.eat_ident()?.0);
        }
        Ok((parts.join("."), span))
    }

    fn eat_comma(&mut self) -> bool {
        let found = self.check_exact(&TokenKind::Comma);
        if found {
            self.advance();
        }
        found
    }
}
