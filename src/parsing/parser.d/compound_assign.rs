/// Affectations composées `x += e`, `x -= e`, `x *= e`, `x /= e`, `x %= e` —
/// réécrites dès le parsing en `x = x op e` (`Stmt::Assign`), la cible étant
/// relue telle quelle : elle ne doit donc contenir aucun appel (E58), sinon
/// cet appel serait évalué deux fois. `-=` produit `BinOp::Remove`, résolu
/// par la sema (soustraction, ou suppression des occurrences sur `string`).
/// Voir docs/EBNF.md §13.

use crate::parsing::ast::*;
use crate::parsing::token::{Span, TokenKind};
use super::types::{Parser, ParseError, ParseResult};

pub(super) fn compound_op(kind: &TokenKind) -> Option<BinOp> {
    match kind {
        TokenKind::PlusEq    => Some(BinOp::Add),
        TokenKind::MinusEq   => Some(BinOp::Remove),
        TokenKind::StarEq    => Some(BinOp::Mul),
        TokenKind::SlashEq   => Some(BinOp::Div),
        TokenKind::PercentEq => Some(BinOp::Mod),
        _ => None,
    }
}

fn op_text(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add    => "+=",
        BinOp::Remove => "-=",
        BinOp::Mul    => "*=",
        BinOp::Div    => "/=",
        _             => "%=",
    }
}

/// Premier appel (ou autre expression à effet de bord) contenu dans `expr`.
fn side_effect(expr: &Expr) -> Option<Span> {
    match expr {
        Expr::Call { span, .. } | Expr::StaticCall { span, .. } | Expr::New { span, .. }
        | Expr::IncDec { span, .. } | Expr::Nameless { span, .. } | Expr::Resolve { span, .. }
        | Expr::Match { span, .. } => Some(span.clone()),
        Expr::Field { object: e, .. } | Expr::Unary { operand: e, .. } | Expr::IsCheck { expr: e, .. } => side_effect(e),
        Expr::Index { object: a, index: b, .. } | Expr::Binary { left: a, right: b, .. } | Expr::Range { start: a, end: b, .. } => {
            side_effect(a).or_else(|| side_effect(b))
        }
        Expr::Array { elements, .. } => elements.iter().find_map(side_effect),
        Expr::Map { entries, .. } => entries.iter().find_map(|(k, v)| side_effect(k).or_else(|| side_effect(v))),
        Expr::Template { parts, .. } => parts.iter().find_map(|p| match p {
            TemplatePartExpr::Expr(e) => side_effect(e),
            _ => None,
        }),
        _ => None,
    }
}

impl Parser {
    /// `target` déjà lu ; le token courant est l'opérateur composé `op`.
    pub(super) fn parse_compound_assign(&mut self, target: Expr, op: BinOp) -> ParseResult<Stmt> {
        let span = self.span();
        self.advance();
        if let Some(call_span) = side_effect(&target) {
            return Err(ParseError::new(
                format!("the target of '{}' cannot contain a call — it would be evaluated twice; store the call result in a variable first", op_text(&op)),
                call_span,
            ));
        }
        let value = self.parse_expr()?;
        let value = Expr::Binary { op, left: Box::new(target.clone()), right: Box::new(value), span: span.clone() };
        Ok(Stmt::Assign { target, value, span })
    }
}
