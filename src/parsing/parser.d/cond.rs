/// Expression conditionnelle `cond then a else b` et sa forme courte
/// `cond then a empty|zero|null` — voir docs/EBNF.md §11.3.
///
/// Niveau de précédence le plus faible (sous `or`), associatif à droite.
/// `empty`/`zero` sont des mots-clés contextuels : identifiants partout,
/// sauf juste après l'expression complète de la branche `then`, où un
/// identifiant ne peut pas suivre une expression.

use crate::parsing::ast::*;
use crate::parsing::token::{Span, TokenKind};
use super::types::{Parser, ParseError, ParseResult};

type Operand = fn(&mut Parser) -> ParseResult<Expr>;

impl Parser {
    /// `operand` : niveau `or` (avec ou sans `is`, voir `parse_expr_no_is`).
    pub(super) fn parse_cond(&mut self, operand: Operand) -> ParseResult<Expr> {
        let cond = operand(self)?;
        if !self.check_exact(&TokenKind::Then) {
            return Ok(cond);
        }
        let span = self.span();
        self.advance();
        self.deepen()?;
        let then_expr = self.parse_cond(operand)?;
        let else_expr = self.parse_cond_else(operand, &span)?;
        Ok(Expr::Cond { cond: Box::new(cond), then_expr: Box::new(then_expr), else_expr, span })
    }

    fn parse_cond_else(&mut self, operand: Operand, then_span: &Span) -> ParseResult<CondElse> {
        if self.check_exact(&TokenKind::Else) {
            self.advance();
            return Ok(CondElse::Value(Box::new(self.parse_cond(operand)?)));
        }
        let default = match self.peek_kind() {
            TokenKind::LitNull => CondDefault::Null,
            TokenKind::Ident(name) if name == "empty" => CondDefault::Empty,
            TokenKind::Ident(name) if name == "zero" => CondDefault::Zero,
            _ => {
                return Err(ParseError::new(
                    "'then' expects 'else <value>', or a default value 'empty', 'zero' or 'null', after its value",
                    then_span.clone(),
                ));
            }
        };
        let span = self.span();
        self.advance();
        self.cond_keywords.push(span.clone());
        Ok(CondElse::Default(default, span))
    }
}
