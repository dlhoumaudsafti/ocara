/// `assertBlocksMainThread(expr)` / `assertIsBlocking(expr)` : tranchées à
/// la compilation, `expr` est vérifiée mais jamais évaluée (savoir si un
/// appel bloque en l'exécutant est indécidable, et ouvrirait réellement la
/// fenêtre). L'appel est analysé : étiquette du builtin appelé
/// (`crate::builtins::blocking`), propagée aux fonctions et méthodes
/// utilisateur par le graphe d'appels — le corps d'une `nameless` n'est pas
/// exécuté à sa création et n'est pas suivi. L'assertion est remplacée par
/// `UnitTest::__staticAssert(ok, message, code)` (`AstRewrites::calls`).
///
/// Types des receveurs : `self`, champs de la classe, paramètres et
/// variables déclarés (typés explicitement en Ocara), `use Classe(...)`.

use std::collections::{HashMap, HashSet};

use crate::builtins::blocking::{label, Blocking};
use crate::parsing::ast::{Block, ClassDecl, ClassMember, Expr, FuncDecl, Literal, Param, Program, Stmt, TemplatePartExpr, Type};
use crate::parsing::token::Span;
use crate::sema::error::SemaError;
use crate::sema::named_args::site_key;
use crate::sema::typecheck::TypeChecker;

pub const ASSERT_BLOCKS_MAIN_THREAD: &str = "assertBlocksMainThread";
pub const ASSERT_IS_BLOCKING: &str = "assertIsBlocking";

/// Codes de `UnitTestException` en cas d'échec.
const CODE_BLOCKS_MAIN_THREAD: i64 = 131;
const CODE_IS_BLOCKING: i64 = 132;

pub fn is_blocking_assert(method: &str) -> bool {
    method == ASSERT_BLOCKS_MAIN_THREAD || method == ASSERT_IS_BLOCKING
}

/// Étiquette d'un appel et chemin jusqu'au builtin (`Server.start → HTTPServer.run`).
type Found = Option<(Blocking, String)>;

fn strongest(a: Found, b: Found) -> Found {
    match (a, b) {
        (Some(x), Some(y)) => Some(if y.0 > x.0 { y } else { x }),
        (x, y) => x.or(y),
    }
}

/// Contexte d'un corps : classe de `self` et types déclarés des locales.
struct Ctx<'p> {
    self_class: Option<&'p str>,
    locals: HashMap<String, String>,
}

pub struct BlockingAnalysis<'p> {
    classes: HashMap<&'p str, &'p ClassDecl>,
    functions: HashMap<&'p str, &'p FuncDecl>,
    memo: HashMap<String, Found>,
    in_progress: HashSet<String>,
}

impl<'p> BlockingAnalysis<'p> {
    pub fn new(program: &'p Program) -> Self {
        BlockingAnalysis {
            classes: program.classes.iter().map(|c| (c.name.as_str(), c)).collect(),
            functions: program.functions.iter().map(|f| (f.name.as_str(), f)).collect(),
            memo: HashMap::new(),
            in_progress: HashSet::new(),
        }
    }

    /// Méthode `method` d'une valeur de classe `class` (héritage compris,
    /// jusqu'à un parent builtin).
    pub fn method(&mut self, class: &str, method: &str) -> Found {
        let mut current = class.to_string();
        loop {
            let Some(decl) = self.classes.get(current.as_str()).copied() else {
                return label(&current, method).map(|b| (b, format!("{}.{}", current, method)));
            };
            let body = decl.members.iter().find_map(|m| match m {
                ClassMember::Method { decl: f, .. } if f.name == method => Some((&f.params, &f.body)),
                ClassMember::Constructor { params, body, .. } if method == "init" => Some((params, body)),
                _ => None,
            });
            if let Some((params, body)) = body {
                let key = format!("{}.{}", decl.name, method);
                return self.callable(key, Some(decl.name.as_str()), params, body);
            }
            current = decl.extends.clone()?;
        }
    }

    pub fn function(&mut self, name: &str) -> Found {
        let f = self.functions.get(name).copied()?;
        self.callable(f.name.clone(), None, &f.params, &f.body)
    }

    fn callable(&mut self, key: String, self_class: Option<&'p str>, params: &'p [Param], body: &'p Block) -> Found {
        if let Some(known) = self.memo.get(&key) {
            return known.clone();
        }
        if !self.in_progress.insert(key.clone()) {
            return None;
        }
        let mut locals: HashMap<String, String> = params.iter()
            .filter_map(|p| class_name(&p.ty).map(|c| (p.name.clone(), c)))
            .collect();
        declared_locals(body, &mut locals);
        let ctx = Ctx { self_class, locals };
        let found = self.block(body, &ctx).map(|(b, path)| (b, format!("{} → {}", key, path)));
        self.in_progress.remove(&key);
        self.memo.insert(key, found.clone());
        found
    }

    fn block(&mut self, block: &'p Block, ctx: &Ctx<'p>) -> Found {
        block.stmts.iter().fold(None, |acc, s| strongest(acc, self.stmt(s, ctx)))
    }

    fn stmt(&mut self, stmt: &'p Stmt, ctx: &Ctx<'p>) -> Found {
        match stmt {
            Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value)
            | Stmt::Raise { value, .. } | Stmt::Emit { value, .. } => self.expr(value, ctx),
            Stmt::Assign { target, value, .. } => strongest(self.expr(target, ctx), self.expr(value, ctx)),
            Stmt::Return { value, .. } | Stmt::Result { value, .. } => value.as_ref().and_then(|v| self.expr(v, ctx)),
            Stmt::Break { .. } | Stmt::Continue { .. } => None,
            Stmt::If { condition, then_block, elseif, else_block, .. } => {
                let mut found = strongest(self.expr(condition, ctx), self.block(then_block, ctx));
                for (c, b) in elseif {
                    found = strongest(found, strongest(self.expr(c, ctx), self.block(b, ctx)));
                }
                match else_block {
                    Some(b) => strongest(found, self.block(b, ctx)),
                    None => found,
                }
            }
            Stmt::While { condition, body, .. } => strongest(self.expr(condition, ctx), self.block(body, ctx)),
            Stmt::ForIn { iter, body, .. } | Stmt::ForMap { iter, body, .. } => strongest(self.expr(iter, ctx), self.block(body, ctx)),
            Stmt::Switch { subject, cases, default, .. } => {
                let mut found = self.expr(subject, ctx);
                for c in cases {
                    found = strongest(found, self.block(&c.body, ctx));
                }
                match default {
                    Some(b) => strongest(found, self.block(b, ctx)),
                    None => found,
                }
            }
            Stmt::Try { body, handlers, .. } => handlers.iter()
                .fold(self.block(body, ctx), |acc, h| strongest(acc, self.block(&h.body, ctx))),
        }
    }

    fn expr(&mut self, expr: &'p Expr, ctx: &Ctx<'p>) -> Found {
        match expr {
            // Corps exécuté plus tard, ailleurs (thread, tâche, rappel).
            Expr::Nameless { .. } => None,
            Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => None,
            Expr::Resolve { expr, .. } => strongest(Some((Blocking::Blocks, "resolve".to_string())), self.expr(expr, ctx)),
            Expr::Call { callee, args, .. } => {
                let target = match callee.as_ref() {
                    Expr::Field { object, field, .. } => strongest(
                        self.expr(object, ctx),
                        self.receiver_class(object, ctx).and_then(|c| self.method(&c, field)),
                    ),
                    Expr::Ident(name, _) => self.function(name),
                    other => self.expr(other, ctx),
                };
                self.exprs(args, ctx, target)
            }
            Expr::StaticCall { class, method, args, .. } => {
                let target = self.static_class(class, ctx).and_then(|c| self.method(&c, method));
                self.exprs(args, ctx, target)
            }
            Expr::New { class, args, .. } => {
                let target = self.method(class, "init");
                self.exprs(args, ctx, target)
            }
            Expr::Field { object, .. } => self.expr(object, ctx),
            Expr::Binary { left, right, .. } | Expr::Range { start: left, end: right, .. } | Expr::Index { object: left, index: right, .. } =>
                strongest(self.expr(left, ctx), self.expr(right, ctx)),
            Expr::Unary { operand: e, .. } | Expr::IsCheck { expr: e, .. } | Expr::IncDec { target: e, .. }
            | Expr::NamedArg { value: e, .. } => self.expr(e, ctx),
            Expr::Array { elements, .. } => self.exprs(elements, ctx, None),
            Expr::Map { entries, .. } => entries.iter()
                .fold(None, |acc, (k, v)| strongest(acc, strongest(self.expr(k, ctx), self.expr(v, ctx)))),
            Expr::Template { parts, .. } => parts.iter().fold(None, |acc, p| match p {
                TemplatePartExpr::Expr(e) => strongest(acc, self.expr(e, ctx)),
                TemplatePartExpr::Literal(_) => acc,
            }),
            Expr::Match { subject, arms, .. } => arms.iter()
                .fold(self.expr(subject, ctx), |acc, a| strongest(acc, self.expr(&a.body, ctx))),
            Expr::Cond { cond, then_expr, else_expr, .. } => {
                let found = strongest(self.expr(cond, ctx), self.expr(then_expr, ctx));
                match else_expr.value() {
                    Some(e) => strongest(found, self.expr(e, ctx)),
                    None => found,
                }
            }
        }
    }

    fn exprs(&mut self, exprs: &'p [Expr], ctx: &Ctx<'p>, init: Found) -> Found {
        exprs.iter().fold(init, |acc, e| strongest(acc, self.expr(e, ctx)))
    }

    /// `self::m()` / `parent::m()` / `Classe::m()`.
    fn static_class(&self, class: &str, ctx: &Ctx<'p>) -> Option<String> {
        match class {
            "self" => ctx.self_class.map(str::to_string),
            "parent" => ctx.self_class.and_then(|c| self.classes.get(c)).and_then(|c| c.extends.clone()),
            other => Some(other.to_string()),
        }
    }

    /// Classe d'un receveur dont le type est connu sans inférence.
    fn receiver_class(&self, object: &Expr, ctx: &Ctx<'p>) -> Option<String> {
        match object {
            Expr::SelfExpr(_) => ctx.self_class.map(str::to_string),
            Expr::ParentExpr(_) => ctx.self_class.and_then(|c| self.classes.get(c)).and_then(|c| c.extends.clone()),
            Expr::Ident(name, _) => ctx.locals.get(name).cloned(),
            Expr::New { class, .. } => Some(class.clone()),
            Expr::Field { object, field, .. } => {
                let owner = self.receiver_class(object, ctx)?;
                self.field_class(&owner, field)
            }
            _ => None,
        }
    }

    fn field_class(&self, class: &str, field: &str) -> Option<String> {
        let mut current = self.classes.get(class).copied();
        while let Some(decl) = current {
            let ty = decl.members.iter().find_map(|m| match m {
                ClassMember::Field { name, ty, .. } if name == field => Some(ty),
                _ => None,
            });
            if let Some(ty) = ty {
                return class_name(ty);
            }
            current = decl.extends.as_deref().and_then(|p| self.classes.get(p).copied());
        }
        None
    }
}

fn class_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Named(name) => Some(name.clone()),
        _ => None,
    }
}

/// Variables et constantes déclarées dans un corps (hors `nameless`), avec
/// leur type de classe.
fn declared_locals(block: &Block, out: &mut HashMap<String, String>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Var { name, ty, .. } | Stmt::Const { name, ty, .. } => {
                if let Some(c) = class_name(ty) {
                    out.insert(name.clone(), c);
                }
            }
            Stmt::If { then_block, elseif, else_block, .. } => {
                declared_locals(then_block, out);
                elseif.iter().for_each(|(_, b)| declared_locals(b, out));
                if let Some(b) = else_block { declared_locals(b, out); }
            }
            Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::ForMap { body, .. } => declared_locals(body, out),
            Stmt::Switch { cases, default, .. } => {
                cases.iter().for_each(|c| declared_locals(&c.body, out));
                if let Some(b) = default { declared_locals(b, out); }
            }
            Stmt::Try { body, handlers, .. } => {
                declared_locals(body, out);
                handlers.iter().for_each(|h| declared_locals(&h.body, out));
            }
            _ => {}
        }
    }
}

impl<'a> TypeChecker<'a> {
    /// `assertBlocksMainThread(expr)` / `assertIsBlocking(expr)` : `expr`
    /// vérifiée (un appel `void` est permis), verdict calculé et
    /// l'assertion réécrite en `UnitTest::__staticAssert`.
    pub(crate) fn check_blocking_assert(&mut self, method: &str, args: &[Expr], span: &Span) -> Type {
        let [expr] = args else {
            self.errors.push(SemaError::WrongArgCount { name: format!("UnitTest::{}", method), expected: 1, found: args.len(), span: span.clone() });
            for a in args { self.infer_expr(a); }
            return Type::Void;
        };
        self.infer_expr(expr);
        let found = self.blocking_of(expr);
        let (ok, message, code) = verdict(method, found);
        let call = Expr::StaticCall {
            class: "UnitTest".into(),
            method: "__staticAssert".into(),
            args: vec![
                Expr::Literal(Literal::Bool(ok), span.clone()),
                Expr::Literal(Literal::String(message), span.clone()),
                Expr::Literal(Literal::Int(code), span.clone()),
            ],
            span: span.clone(),
            type_args: Vec::new(),
        };
        self.rewrites.calls.insert(site_key(span), call);
        Type::Void
    }

    /// Étiquette de l'expression de l'assertion. Le receveur d'un appel de
    /// méthode est typé par la sema (variable locale du test comprise).
    fn blocking_of(&mut self, expr: &Expr) -> Found {
        let Some(program) = self.program else { return None };
        let mut analysis = BlockingAnalysis::new(program);
        match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Field { object, field, .. } => {
                    let (errors, warnings) = (self.errors.len(), self.warnings.len());
                    let receiver = self.infer_expr(object);
                    self.errors.truncate(errors);
                    self.warnings.truncate(warnings);
                    class_name(&receiver).and_then(|c| analysis.method(&c, field))
                }
                Expr::Ident(name, _) => analysis.function(name),
                _ => None,
            },
            Expr::StaticCall { class, method, .. } => {
                let ctx = Ctx { self_class: self.current_class.as_deref(), locals: HashMap::new() };
                let class = analysis.static_class(class, &ctx);
                class.and_then(|c| analysis.method(&c, method))
            }
            Expr::Resolve { .. } => Some((Blocking::Blocks, "resolve".to_string())),
            _ => None,
        }
    }
}

fn verdict(method: &str, found: Found) -> (bool, String, i64) {
    match (method, found) {
        (ASSERT_BLOCKS_MAIN_THREAD, Some((Blocking::MainThread, path))) =>
            (true, format!("{}: {} occupies the main thread", method, path), CODE_BLOCKS_MAIN_THREAD),
        (ASSERT_BLOCKS_MAIN_THREAD, Some((Blocking::Blocks, path))) =>
            (false, format!("{}: {} blocks the calling thread but does not occupy the main thread", method, path), CODE_BLOCKS_MAIN_THREAD),
        (ASSERT_BLOCKS_MAIN_THREAD, None) =>
            (false, format!("{}: the expression never blocks", method), CODE_BLOCKS_MAIN_THREAD),
        (_, Some((_, path))) => (true, format!("{}: {} blocks the calling thread", method, path), CODE_IS_BLOCKING),
        (_, None) => (false, format!("{}: the expression never blocks", method), CODE_IS_BLOCKING),
    }
}
