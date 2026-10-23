//! Parcours d'un corps pour `self_borrow` : ce qu'il peut libérer de ce que
//! tient son appelant.

use std::collections::{HashMap, HashSet};

use crate::parsing::ast::{Block, Expr, Param, Stmt, TemplatePartExpr, Type};

use super::self_borrow::Calls;

pub(super) fn assigned_names(body: &Block, out: &mut HashSet<String>) {
    for stmt in &body.stmts {
        match stmt {
            Stmt::Assign { target: Expr::Ident(name, _), .. } => {
                out.insert(name.clone());
            }
            Stmt::If { then_block, elseif, else_block, .. } => {
                assigned_names(then_block, out);
                elseif.iter().for_each(|(_, b)| assigned_names(b, out));
                else_block.iter().for_each(|b| assigned_names(b, out));
            }
            Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::ForMap { body, .. } => assigned_names(body, out),
            Stmt::Switch { cases, default, .. } => {
                cases.iter().for_each(|c| assigned_names(&c.body, out));
                default.iter().for_each(|b| assigned_names(b, out));
            }
            _ => {}
        }
    }
}

/// Builtin qui ne rappelle pas de code Ocara et ne touche à aucun argument.
fn pure_builtin(class: &str, method: &str) -> bool {
    match class {
        "Math" | "String" => true,
        "Convert" => matches!(method,
            "strToInt" | "strToFloat" | "strToBool" | "intToStr" | "intToFloat" | "intToBool"
            | "floatToStr" | "floatToInt" | "floatToBool" | "boolToStr" | "boolToInt" | "boolToFloat"),
        _ => false,
    }
}

pub(super) struct Scan<'a> {
    calls: &'a Calls<'a>,
    /// Classe déclarée de chaque locale/paramètre ; `None` : inconnue, ou
    /// nom lié plusieurs fois avec des types différents.
    classes: HashMap<String, Option<String>>,
    /// Aucune écriture de champ, même de `self`.
    strict: bool,
    /// Ni `nameless` ni `try`.
    pub(super) closed: bool,
}

impl<'a> Scan<'a> {
    pub(super) fn new(params: &[Param], body: &Block, calls: &'a Calls<'a>, strict: bool) -> Self {
        let mut scan = Scan { calls, classes: HashMap::new(), strict, closed: false };
        for p in params {
            scan.bind(&p.name, if p.is_variadic { None } else { Some(&p.ty) });
        }
        scan.collect(body);
        scan
    }

    fn bind(&mut self, name: &str, ty: Option<&Type>) {
        let class = match ty {
            Some(Type::Named(n)) => Some(n.clone()),
            _ => None,
        };
        self.classes.entry(name.to_string())
            .and_modify(|c| if *c != class { *c = None })
            .or_insert(class);
    }

    fn collect(&mut self, body: &Block) {
        for stmt in &body.stmts {
            match stmt {
                Stmt::Var { name, ty, .. } | Stmt::Const { name, ty, .. } => self.bind(name, Some(ty)),
                Stmt::If { then_block, elseif, else_block, .. } => {
                    self.collect(then_block);
                    elseif.iter().for_each(|(_, b)| self.collect(b));
                    else_block.iter().for_each(|b| self.collect(b));
                }
                Stmt::While { body, .. } => self.collect(body),
                Stmt::ForIn { var, body, .. } => {
                    self.bind(var, None);
                    self.collect(body);
                }
                Stmt::ForMap { key, value, body, .. } => {
                    self.bind(key, None);
                    self.bind(value, None);
                    self.collect(body);
                }
                Stmt::Switch { cases, default, .. } => {
                    cases.iter().for_each(|c| self.collect(&c.body));
                    default.iter().for_each(|b| self.collect(b));
                }
                Stmt::Try { body, handlers, .. } => {
                    self.collect(body);
                    for h in handlers {
                        self.bind(&h.binding, None);
                        self.collect(&h.body);
                    }
                }
                _ => {}
            }
        }
    }

    fn class_of(&self, name: &str) -> Option<&str> {
        self.classes.get(name).and_then(|c| c.as_deref())
    }

    pub(super) fn block(&self, body: &Block) -> bool {
        body.stmts.iter().all(|s| self.stmt(s))
    }

    fn stmt(&self, stmt: &Stmt) -> bool {
        let block = |b: &Block| self.block(b);
        let expr = |e: &Expr| self.expr(e);
        match stmt {
            Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value) | Stmt::Raise { value, .. } => expr(value),
            Stmt::Return { value, .. } | Stmt::Result { value, .. } => value.as_ref().is_none_or(expr),
            Stmt::Break { .. } | Stmt::Continue { .. } => true,
            Stmt::If { condition, then_block, elseif, else_block, .. } =>
                expr(condition) && block(then_block)
                    && elseif.iter().all(|(c, b)| expr(c) && block(b))
                    && else_block.as_ref().is_none_or(block),
            Stmt::While { condition, body, .. } => expr(condition) && block(body),
            Stmt::ForIn { iter, body, .. } | Stmt::ForMap { iter, body, .. } => expr(iter) && block(body),
            Stmt::Switch { subject, cases, default, .. } =>
                expr(subject) && cases.iter().all(|c| block(&c.body)) && default.as_ref().is_none_or(block),
            Stmt::Try { body, handlers, .. } => !self.closed && block(body) && handlers.iter().all(|h| block(&h.body)),
            Stmt::Assign { target, value, .. } => expr(value) && match target {
                Expr::Ident(..) => true,
                Expr::Field { object, field, .. } => match object.as_ref() {
                    Expr::SelfExpr(_) => !self.strict || self.calls.self_class.is_some_and(|c| (self.calls.scalar_field)(c, field)),
                    Expr::Ident(name, _) => self.class_of(name).is_some_and(|c| (self.calls.scalar_field)(c, field)),
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    }

    fn expr(&self, expr: &Expr) -> bool {
        let safe = |e: &Expr| self.expr(e);
        match expr {
            Expr::Call { callee, args, .. } => args.iter().all(safe) && match callee.as_ref() {
                Expr::Field { object, field, .. } => match object.as_ref() {
                    Expr::SelfExpr(_) => (self.calls.self_call)(field),
                    Expr::Ident(name, _) => self.class_of(name).is_some_and(|c| (self.calls.pure_call)(c, field)),
                    _ => false,
                },
                Expr::Ident(name, _) => !self.classes.contains_key(name.as_str()) && (self.calls.pure_fn)(name),
                _ => false,
            },
            Expr::StaticCall { class, method, args, .. } =>
                !(self.calls.user_type)(class) && pure_builtin(class, method) && args.iter().all(safe),
            Expr::Nameless { .. } => !self.closed,
            Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_)
            | Expr::StaticConst { .. } => true,
            Expr::Field { object, .. } => safe(object),
            Expr::Binary { left, right, .. } => safe(left) && safe(right),
            Expr::Unary { operand, .. } => safe(operand),
            Expr::IsCheck { expr, .. } => safe(expr),
            Expr::IncDec { target, .. } => safe(target),
            Expr::Index { object, index, .. } => safe(object) && safe(index),
            Expr::Range { start, end, .. } => safe(start) && safe(end),
            Expr::Array { elements, .. } => elements.iter().all(safe),
            Expr::Map { entries, .. } => entries.iter().all(|(k, v)| safe(k) && safe(v)),
            Expr::Template { parts, .. } => parts.iter().all(|p| match p {
                TemplatePartExpr::Expr(e) => safe(e),
                _ => true,
            }),
            Expr::Match { subject, arms, .. } => safe(subject) && arms.iter().all(|a| safe(&a.body)),
            _ => false,
        }
    }
}
