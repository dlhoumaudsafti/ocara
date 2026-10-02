/// Initialiseur inline de `property` — `public property nom:Type = expr` —
/// voir docs/roadmap.d/langage-property-initializer.md et `docs/EBNF.md`
/// §16.3.
///
/// Désucré dès le parsing : chaque initialiseur devient `self.nom = expr`
/// en TÊTE du corps de `init()`, dans l'ordre de déclaration (un `init()`
/// sans paramètre est synthétisé s'il n'y en a pas — complété par
/// `core::property_init` si un ancêtre a un constructeur). Toute la suite
/// du compilateur (résolution d'alias, sema, analyse des ressources et de
/// l'échappement, lowering) voit donc une affectation ordinaire.
///
/// v1 : l'initialiseur ne peut pas référencer `self`/`parent` (E56) — aucun
/// ordre d'initialisation ENTRE properties à définir.

use crate::parsing::ast::*;
use crate::parsing::token::Span;
use super::types::{Parser, ParseError, ParseResult};

impl Parser {
    pub(super) fn parse_property_initializer(&mut self, field: &str) -> ParseResult<Expr> {
        let value = self.parse_expr()?;
        if let Some(span) = self_reference(&value) {
            return Err(ParseError::new(
                format!("initializer of property '{}' cannot use 'self' or 'parent' — it is evaluated before init(), independently of the other properties; assign it in init() instead", field),
                span,
            ));
        }
        Ok(value)
    }

    /// Injecte les initialiseurs du corps qui vient d'être lu dans `init()` ;
    /// `true` si cet `init()` a dû être synthétisé.
    pub(super) fn inject_property_initializers(&mut self, members: &mut Vec<ClassMember>, span: &Span) -> bool {
        let assigns: Vec<Stmt> = self.property_initializers.drain(..)
            .map(|(name, value, name_span)| Stmt::Assign {
                target: Expr::Field { object: Box::new(Expr::SelfExpr(name_span.clone())), field: name, span: name_span.clone() },
                value,
                span: name_span,
            })
            .collect();
        if assigns.is_empty() {
            return false;
        }
        if let Some(ClassMember::Constructor { body, .. }) = members.iter_mut().find(|m| matches!(m, ClassMember::Constructor { .. })) {
            body.stmts.splice(0..0, assigns);
            return false;
        }
        members.push(ClassMember::Constructor {
            params: Vec::new(),
            body: Block { stmts: assigns, span: span.clone() },
            span: span.clone(),
        });
        true
    }
}

/// Position du premier `self`/`parent` de `expr`, closures comprises.
fn self_reference(expr: &Expr) -> Option<Span> {
    let mut found = None;
    visit(expr, &mut |e| {
        if found.is_none() {
            if let Expr::SelfExpr(span) | Expr::ParentExpr(span) = e {
                found = Some(span.clone());
            }
            if let Expr::StaticCall { class, span, .. } | Expr::StaticConst { class, span, .. } = e {
                if class == "<self>" || class == "<parent>" {
                    found = Some(span.clone());
                }
            }
        }
    });
    found
}

fn visit(expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);
    match expr {
        Expr::Field { object: e, .. } | Expr::Unary { operand: e, .. } | Expr::Resolve { expr: e, .. }
        | Expr::IsCheck { expr: e, .. } | Expr::IncDec { target: e, .. } | Expr::NamedArg { value: e, .. } => visit(e, f),
        Expr::Binary { left: a, right: b, .. } | Expr::Index { object: a, index: b, .. } | Expr::Range { start: a, end: b, .. } => {
            visit(a, f);
            visit(b, f);
        }
        Expr::Call { callee, args, .. } => {
            visit(callee, f);
            args.iter().for_each(|a| visit(a, f));
        }
        Expr::StaticCall { args, .. } | Expr::New { args, .. } => args.iter().for_each(|a| visit(a, f)),
        Expr::Array { elements, .. } => elements.iter().for_each(|e| visit(e, f)),
        Expr::Map { entries, .. } => entries.iter().for_each(|(k, v)| { visit(k, f); visit(v, f); }),
        Expr::Template { parts, .. } => parts.iter().for_each(|p| if let TemplatePartExpr::Expr(e) = p { visit(e, f) }),
        Expr::Match { subject, arms, .. } => {
            visit(subject, f);
            arms.iter().for_each(|a| visit(&a.body, f));
        }
        Expr::Nameless { body, .. } => body.stmts.iter().for_each(|s| visit_stmt(s, f)),
        Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => {}
    }
}

fn visit_stmt(stmt: &Stmt, f: &mut impl FnMut(&Expr)) {
    match stmt {
        Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value)
        | Stmt::Raise { value, .. } | Stmt::Emit { value, .. } => visit(value, f),
        Stmt::Assign { target, value, .. } => { visit(target, f); visit(value, f); }
        Stmt::Return { value: Some(v), .. } | Stmt::Result { value: Some(v), .. } => visit(v, f),
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            visit(condition, f);
            then_block.stmts.iter().for_each(|s| visit_stmt(s, f));
            for (c, b) in elseif { visit(c, f); b.stmts.iter().for_each(|s| visit_stmt(s, f)); }
            if let Some(b) = else_block { b.stmts.iter().for_each(|s| visit_stmt(s, f)); }
        }
        Stmt::While { condition: e, body, .. } | Stmt::ForIn { iter: e, body, .. } | Stmt::ForMap { iter: e, body, .. } => {
            visit(e, f);
            body.stmts.iter().for_each(|s| visit_stmt(s, f));
        }
        Stmt::Switch { subject, cases, default, .. } => {
            visit(subject, f);
            cases.iter().for_each(|c| c.body.stmts.iter().for_each(|s| visit_stmt(s, f)));
            if let Some(b) = default { b.stmts.iter().for_each(|s| visit_stmt(s, f)); }
        }
        Stmt::Try { body, handlers, .. } => {
            body.stmts.iter().for_each(|s| visit_stmt(s, f));
            handlers.iter().for_each(|h| h.body.stmts.iter().for_each(|s| visit_stmt(s, f)));
        }
        _ => {}
    }
}
