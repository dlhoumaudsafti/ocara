/// Parcours de l'AST partagés par l'analyse de propriété des objets
/// (`object_owners`, `object_facts`) : retours d'un corps, sous-blocs d'une
/// instruction, comptage des références (identifiants, et champs sous la
/// clé `#champ`), reconnaissance d'un conteneur d'objets.
use std::collections::{HashMap, HashSet};

use crate::parsing::ast::{Block, Expr, Stmt, TemplatePartExpr, Type};

pub(crate) fn collect_returns<'b>(block: &'b Block, out: &mut Vec<&'b Expr>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Return { value: Some(v), .. } => out.push(v),
            Stmt::Return { value: None, .. } => out.push(&NULL_EXPR),
            _ => for_each_block(stmt, &mut |b| collect_returns(b, out)),
        }
    }
}

static NULL_EXPR: Expr = Expr::Literal(crate::parsing::ast::Literal::Null, crate::parsing::token::Span { line: 0, col: 0, file: None, runtime_ctx: None });

pub(crate) fn for_each_block<'b>(stmt: &'b Stmt, f: &mut dyn FnMut(&'b Block)) {
    match stmt {
        Stmt::If { then_block, elseif, else_block, .. } => {
            f(then_block);
            elseif.iter().for_each(|(_, b)| f(b));
            if let Some(b) = else_block { f(b); }
        }
        Stmt::Switch { cases, default, .. } => {
            cases.iter().for_each(|c| f(&c.body));
            if let Some(b) = default { f(b); }
        }
        Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::ForMap { body, .. } => f(body),
        Stmt::Try { body, handlers, .. } => {
            f(body);
            handlers.iter().for_each(|h| f(&h.body));
        }
        _ => {}
    }
}

pub(crate) fn count_refs_block(block: &Block, refs: &mut HashMap<String, usize>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value) | Stmt::Raise { value, .. } | Stmt::Emit { value, .. } => count_refs(value, refs),
            Stmt::Assign { target, value, .. } => { count_refs(target, refs); count_refs(value, refs); }
            Stmt::Return { value: Some(v), .. } | Stmt::Result { value: Some(v), .. } => count_refs(v, refs),
            Stmt::If { condition: c, .. } | Stmt::While { condition: c, .. } | Stmt::Switch { subject: c, .. } => count_refs(c, refs),
            Stmt::ForIn { iter, .. } | Stmt::ForMap { iter, .. } => count_refs(iter, refs),
            _ => {}
        }
        for_each_block(stmt, &mut |b| count_refs_block(b, refs));
    }
}

fn count_refs(expr: &Expr, refs: &mut HashMap<String, usize>) {
    match expr {
        Expr::Ident(name, _) => *refs.entry(name.clone()).or_default() += 1,
        Expr::Call { callee, args, .. } => { count_refs(callee, refs); args.iter().for_each(|a| count_refs(a, refs)); }
        Expr::StaticCall { args, .. } | Expr::New { args, .. } | Expr::Array { elements: args, .. } => args.iter().for_each(|a| count_refs(a, refs)),
        Expr::Map { entries, .. } => entries.iter().for_each(|(k, v)| { count_refs(k, refs); count_refs(v, refs); }),
        // Un champ compte sous la clé `#champ` (voir le cas `Nameless` de `Scan::expr`).
        Expr::Field { object, field, .. } => {
            *refs.entry(format!("#{}", field)).or_default() += 1;
            count_refs(object, refs);
        }
        Expr::Unary { operand: e, .. } | Expr::Resolve { expr: e, .. }
        | Expr::IsCheck { expr: e, .. } | Expr::IncDec { target: e, .. } | Expr::NamedArg { value: e, .. } => count_refs(e, refs),
        Expr::Binary { left: a, right: b, .. } | Expr::Index { object: a, index: b, .. } | Expr::Range { start: a, end: b, .. } => {
            count_refs(a, refs);
            count_refs(b, refs);
        }
        Expr::Template { parts, .. } => parts.iter().for_each(|p| if let TemplatePartExpr::Expr(e) = p { count_refs(e, refs) }),
        Expr::Match { subject, arms, .. } => { count_refs(subject, refs); arms.iter().for_each(|a| count_refs(&a.body, refs)); }
        Expr::Nameless { body, .. } => {
            let mut inner = HashSet::new();
            crate::sema::escape::collect_ident_refs(body, &mut inner);
            inner.into_iter().for_each(|n| *refs.entry(n).or_default() += 2);
        }
        Expr::Literal(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => {}
    }
}

/// `array<Classe>` ou `map<K, Classe>` (classe utilisateur).
pub fn object_elem_class(ty: &Type) -> Option<&str> {
    match ty {
        Type::Array(inner) | Type::Map(_, inner) => match inner.as_ref() {
            Type::Named(n) => Some(n.as_str()),
            _ => None,
        },
        _ => None,
    }
}

pub fn is_object_container(ty: &Type) -> bool {
    object_elem_class(ty).is_some()
}
