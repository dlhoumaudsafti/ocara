/// Parsing de `struct` — agrégat de données : variante déclarative de
/// `class` (même représentation runtime, tas via `use`), limitée aux champs
/// et constantes, dont le constructeur est généré depuis les champs. Voir
/// docs/roadmap.d/langage-struct-value-type.md et `docs/EBNF.md` §16.7.
///
/// Le constructeur synthétisé ici ne couvre que les champs PROPRES : ceux
/// d'un struct parent (`extends`), qui peut vivre dans un autre fichier, sont
/// ajoutés en tête par `core::structs` une fois tous les imports chargés.

use crate::parsing::ast::*;
use crate::parsing::token::{Span, TokenKind};
use super::types::{Parser, ParseError, ParseResult};

impl Parser {
    pub(super) fn parse_struct(&mut self) -> ParseResult<ClassDecl> {
        let span = self.span();
        self.eat(&TokenKind::Struct)?;
        let (name, _) = self.eat_ident()?;

        let extends = if self.check_exact(&TokenKind::Extends) {
            self.advance();
            Some(self.eat_type_name()?)
        } else {
            None
        };
        if self.check_exact(&TokenKind::Modules) || self.check_exact(&TokenKind::Implements) {
            return Err(ParseError::new(
                format!("struct '{}' cannot use 'modules' or 'implements' — a struct only holds data; use a class", name),
                self.span(),
            ));
        }

        self.eat(&TokenKind::LBrace)?;
        let mut members = Vec::new();
        let mut params = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            match self.parse_struct_member(&name)? {
                StructMember::Field { member, param } => {
                    members.push(member);
                    params.push(param);
                }
                StructMember::Const(member) => members.push(member),
            }
        }
        self.eat(&TokenKind::RBrace)?;

        let body = Block {
            stmts: params.iter().map(|p| assign_self_field(&p.name, &p.span)).collect(),
            span: span.clone(),
        };
        members.push(ClassMember::Constructor { params, body, span: span.clone() });

        Ok(ClassDecl { name, extends, modules: Vec::new(), implements: Vec::new(), members, span, is_struct: true, implicit_init: false })
    }

    /// `[vis] const NAME:T = expr` ou `[vis] [property] name:T [= expr]` —
    /// visibilité optionnelle (public par défaut). `private` est accepté ici
    /// pour être rejeté avec un diagnostic dédié (E51, `core::structs`).
    fn parse_struct_member(&mut self, struct_name: &str) -> ParseResult<StructMember> {
        let span = self.span();
        let vis = match self.peek_kind() {
            TokenKind::Public    => { self.advance(); Visibility::Public }
            TokenKind::Protected => { self.advance(); Visibility::Protected }
            TokenKind::Private   => { self.advance(); Visibility::Private }
            _ => Visibility::Public,
        };

        if matches!(self.peek_kind(), TokenKind::Method | TokenKind::Async | TokenKind::Static | TokenKind::Init) {
            return Err(ParseError::new(
                format!("struct '{}' can only declare fields and constants — methods and 'init' belong in a class (the constructor of a struct is generated from its fields)", struct_name),
                self.span(),
            ));
        }

        if self.check_exact(&TokenKind::Const) {
            self.advance();
            let (name, _) = self.eat_ident()?;
            self.eat(&TokenKind::Colon)?;
            let ty = self.parse_type()?;
            self.eat(&TokenKind::Eq)?;
            let value = self.parse_expr()?;
            return Ok(StructMember::Const(ClassMember::Const { vis, name, ty, value, span }));
        }

        if self.check_exact(&TokenKind::Property) {
            self.advance();
        }
        let (name, name_span) = self.eat_ident()?;
        self.eat(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        let default_value = if self.check_exact(&TokenKind::Eq) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        let param = Param { name: name.clone(), ty: ty.clone(), default_value, is_variadic: false, span: name_span };
        Ok(StructMember::Field {
            member: ClassMember::Field { vis, mutable: true, name, ty, span },
            param,
        })
    }
}

enum StructMember {
    /// Le champ, et le paramètre correspondant du constructeur généré.
    Field { member: ClassMember, param: Param },
    Const(ClassMember),
}

/// `self.<name> = <name>`
pub(crate) fn assign_self_field(name: &str, span: &Span) -> Stmt {
    Stmt::Assign {
        target: Expr::Field { object: Box::new(Expr::SelfExpr(span.clone())), field: name.to_string(), span: span.clone() },
        value: Expr::Ident(name.to_string(), span.clone()),
        span: span.clone(),
    }
}
