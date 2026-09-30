/// Réinjection dans l'AST des appels à arguments nommés résolus par la sema
/// (voir `crate::sema::named_args`) : chaque liste `args` commençant par un
/// `Expr::NamedArg` est remplacée par sa forme positionnelle. Tourne juste
/// après une sema sans erreur, avant la monomorphisation et le lowering —
/// qui ne voient donc jamais d'argument nommé.
///
/// Code jamais parcouru par la sema (corps d'un `generic`) : la cible est
/// alors résolue syntaxiquement quand elle ne dépend d'aucun type (fonction
/// libre, `Classe::m(...)`, `self::m(...)`, `self.m(...)`, `use X(...)`) ;
/// tout autre argument nommé non résolu est une erreur plutôt que passé au
/// lowering.

use std::collections::HashMap;
use crate::parsing::ast::{Block, ClassMember, Expr, Param, Program, Stmt, TemplatePartExpr};
use crate::parsing::token::Span;
use crate::sema::named_args::{collect_callable_params, reorder, site_key, ArgSiteKey, CallTarget};

/// Position et message de la première erreur rencontrée.
pub type NamedArgError = (Span, String);

pub fn rewrite_named_args(
    program: &mut Program,
    rewrites: &HashMap<ArgSiteKey, Vec<Expr>>,
) -> Result<(), NamedArgError> {
    let targets: HashMap<String, CallTarget> = collect_callable_params(program)
        .into_iter()
        .map(|(key, params)| (key.clone(), CallTarget::from_params(key, params)))
        .collect();
    let rw = |owner: Option<&str>| Rewriter { rewrites, targets: &targets, owner: owner.map(str::to_string) };

    let free = rw(None);
    for f in &mut program.functions {
        free.params(&mut f.params)?;
        free.block(&mut f.body)?;
    }
    for (owner, members) in program.classes.iter_mut().map(|c| (&c.name, &mut c.members))
        .chain(program.generics.iter_mut().map(|g| (&g.name, &mut g.members)))
        .chain(program.modules.iter_mut().map(|m| (&m.name, &mut m.members)))
    {
        let scoped = rw(Some(owner));
        for member in members {
            scoped.member(member)?;
        }
    }
    for c in &mut program.consts {
        free.expr(&mut c.value)?;
    }
    for rb in &mut program.runtime_blocks {
        for s in &mut rb.statements {
            free.stmt(s)?;
        }
    }
    Ok(())
}

struct Rewriter<'r> {
    rewrites: &'r HashMap<ArgSiteKey, Vec<Expr>>,
    targets:  &'r HashMap<String, CallTarget>,
    /// Classe/générique/module dont on parcourt les membres (`self`).
    owner:    Option<String>,
}

impl Rewriter<'_> {
    fn member(&self, member: &mut ClassMember) -> Result<(), NamedArgError> {
        match member {
            ClassMember::Method { decl, .. } => {
                self.params(&mut decl.params)?;
                self.block(&mut decl.body)
            }
            ClassMember::Constructor { params, body, .. } => {
                self.params(params)?;
                self.block(body)
            }
            ClassMember::Const { value, .. } => self.expr(value),
            ClassMember::Field { .. } => Ok(()),
        }
    }

    fn params(&self, params: &mut [Param]) -> Result<(), NamedArgError> {
        params.iter_mut()
            .filter_map(|p| p.default_value.as_mut())
            .try_for_each(|d| self.expr(d))
    }

    fn block(&self, block: &mut Block) -> Result<(), NamedArgError> {
        block.stmts.iter_mut().try_for_each(|s| self.stmt(s))
    }

    fn stmt(&self, stmt: &mut Stmt) -> Result<(), NamedArgError> {
        match stmt {
            Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value)
            | Stmt::Raise { value, .. } | Stmt::Emit { value, .. } => self.expr(value),
            Stmt::Assign { target, value, .. } => {
                self.expr(target)?;
                self.expr(value)
            }
            Stmt::If { condition, then_block, elseif, else_block, .. } => {
                self.expr(condition)?;
                self.block(then_block)?;
                for (cond, blk) in elseif {
                    self.expr(cond)?;
                    self.block(blk)?;
                }
                else_block.as_mut().map_or(Ok(()), |b| self.block(b))
            }
            Stmt::Switch { subject, cases, default, .. } => {
                self.expr(subject)?;
                for c in cases {
                    self.block(&mut c.body)?;
                }
                default.as_mut().map_or(Ok(()), |b| self.block(b))
            }
            Stmt::While { condition: e, body, .. } | Stmt::ForIn { iter: e, body, .. } | Stmt::ForMap { iter: e, body, .. } => {
                self.expr(e)?;
                self.block(body)
            }
            Stmt::Return { value, .. } | Stmt::Result { value, .. } => {
                value.as_mut().map_or(Ok(()), |v| self.expr(v))
            }
            Stmt::Try { body, handlers, .. } => {
                self.block(body)?;
                handlers.iter_mut().try_for_each(|h| self.block(&mut h.body))
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => Ok(()),
        }
    }

    /// `target_key` : clé de la cible dans `targets` quand elle se déduit de
    /// la seule syntaxe de l'appel (repli hors sema, voir doc de module).
    fn args(&self, args: &mut Vec<Expr>, target_key: Option<String>) -> Result<(), NamedArgError> {
        if let Some(Expr::NamedArg { name, span, .. }) = args.first() {
            let positional = match self.rewrites.get(&site_key(span)) {
                Some(positional) => positional.clone(),
                None => match target_key.as_ref().and_then(|k| self.targets.get(k)) {
                    Some(target) => reorder(args, target).map_err(|e| (e.span().clone(), e.message()))?,
                    None => return Err((span.clone(), unresolved_message(name))),
                },
            };
            *args = positional;
        }
        args.iter_mut().try_for_each(|a| self.expr(a))
    }

    fn owner_key(&self, member: &str) -> Option<String> {
        self.owner.as_ref().map(|owner| format!("{}::{}", owner, member))
    }

    fn expr(&self, expr: &mut Expr) -> Result<(), NamedArgError> {
        match expr {
            Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => Ok(()),
            Expr::Call { callee, args, .. } => {
                self.expr(callee)?;
                let target_key = match callee.as_ref() {
                    Expr::Ident(name, _) => Some(name.clone()),
                    Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::SelfExpr(_)) => self.owner_key(field),
                    _ => None,
                };
                self.args(args, target_key)
            }
            Expr::StaticCall { class, method, args, .. } => {
                let target_key = match class.as_str() {
                    "<self>" => self.owner_key(method),
                    "<parent>" => None,
                    _ => Some(format!("{}::{}", class, method)),
                };
                self.args(args, target_key)
            }
            Expr::New { class, args, .. } => self.args(args, Some(format!("{}::init", class))),
            Expr::NamedArg { name, span, .. } => Err((span.clone(), unresolved_message(name))),
            Expr::Field { object: e, .. } | Expr::Unary { operand: e, .. }
            | Expr::Resolve { expr: e, .. } | Expr::IsCheck { expr: e, .. } | Expr::IncDec { target: e, .. } => self.expr(e),
            Expr::Binary { left: a, right: b, .. } | Expr::Index { object: a, index: b, .. } | Expr::Range { start: a, end: b, .. } => {
                self.expr(a)?;
                self.expr(b)
            }
            Expr::Array { elements, .. } => elements.iter_mut().try_for_each(|e| self.expr(e)),
            Expr::Map { entries, .. } => entries.iter_mut().try_for_each(|(k, v)| {
                self.expr(k)?;
                self.expr(v)
            }),
            Expr::Template { parts, .. } => parts.iter_mut().try_for_each(|p| match p {
                TemplatePartExpr::Expr(e) => self.expr(e),
                _ => Ok(()),
            }),
            Expr::Match { subject, arms, .. } => {
                self.expr(subject)?;
                arms.iter_mut().try_for_each(|arm| self.expr(&mut arm.body))
            }
            Expr::Nameless { params, body, .. } => {
                self.params(params)?;
                self.block(body)
            }
        }
    }
}

fn unresolved_message(name: &str) -> String {
    format!("named argument '{}' cannot be resolved here: this call's target depends on a type not known outside semantic analysis (e.g. an instance method called inside a 'generic' body) — pass the arguments positionally", name)
}
