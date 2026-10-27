/// Réinjection dans l'AST des réécritures décidées par la sema
/// (`AstRewrites`) : chaque liste `args` commençant par un `Expr::NamedArg`
/// est remplacée par sa forme positionnelle (voir `crate::sema::named_args`),
/// et chaque appel de sucre `Convert` (`s.toInt()`) par l'appel statique
/// correspondant (voir `crate::sema::convert_sugar`), et chaque `truc.A` sur
/// une map par `truc["A"]` (voir `crate::sema::map_dot`). Tourne juste après
/// une sema sans erreur, avant la monomorphisation et le lowering — qui ne
/// voient donc jamais ni argument nommé, ni sucre `Convert`, ni notation
/// pointée sur une map.
///
/// Tout le code est parcouru par la sema (corps des `generic`/`module`
/// compris, voir `crate::sema::generic_check`) : un argument nommé sans
/// résolution enregistrée est une erreur plutôt que passé au lowering.

use crate::parsing::ast::{BinOp, Block, ClassMember, Expr, ImportDecl, Literal, Param, Program, Stmt, TemplatePartExpr};
use crate::parsing::token::Span;
use crate::sema::named_args::{site_key, AstRewrites};

/// Position et message de la première erreur rencontrée.
pub type NamedArgError = (Span, String);

pub fn rewrite_program(program: &mut Program, rewrites: &AstRewrites) -> Result<(), NamedArgError> {
    if !rewrites.calls.is_empty() || !rewrites.mixed_to_string.is_empty() || !rewrites.mixed_checks.is_empty()
        || !rewrites.mixed_reads.is_empty() || !rewrites.mixed_writes.is_empty()
    {
        ensure_builtin_import(program, "Convert");
    }
    if !rewrites.string_removals.is_empty() {
        ensure_builtin_import(program, "String");
    }
    let free = Rewriter { rewrites };
    for f in &mut program.functions {
        free.params(&mut f.params)?;
        free.block(&mut f.body)?;
    }
    for members in program.classes.iter_mut().map(|c| &mut c.members)
        .chain(program.generics.iter_mut().map(|g| &mut g.members))
        .chain(program.modules.iter_mut().map(|m| &mut m.members))
    {
        for member in members {
            free.member(member)?;
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
    rewrites: &'r AstRewrites,
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
        // `x[k] = v`/`x.k = v` sur un `mixed` : `Convert::mixedSet(x, k, v)`
        // (voir `crate::sema::mixed_access`).
        if let Stmt::Assign { target, value, .. } = stmt {
            let span = target.span().clone();
            if self.rewrites.mixed_writes.contains(&site_key(&span)) {
                if let Some((mut object, mut key)) = take_access(target) {
                    self.expr(&mut object)?;
                    self.expr(&mut key)?;
                    self.expr(value)?;
                    let args = vec![object, key, value.clone()];
                    *stmt = Stmt::Expr(Expr::StaticCall { class: "Convert".into(), method: "mixedSet".into(), args, span, type_args: Vec::new() });
                    return Ok(());
                }
            }
        }
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

    fn args(&self, args: &mut Vec<Expr>) -> Result<(), NamedArgError> {
        if let Some(Expr::NamedArg { name, span, .. }) = args.first() {
            match self.rewrites.args.get(&site_key(span)) {
                Some(positional) => *args = positional.clone(),
                None => return Err((span.clone(), unresolved_message(name))),
            }
        }
        args.iter_mut().try_for_each(|a| self.expr(a))
    }

    fn expr(&self, expr: &mut Expr) -> Result<(), NamedArgError> {
        // `mixed` attendu comme `string` : `Convert::mixedToStr(expr)` (voir
        // `crate::sema::mixed_string`). L'expression d'origine est réécrite
        // une seule fois (`expr_body`), puis enveloppée.
        let key = (site_key(expr.span()), std::mem::discriminant(&*expr));
        if self.rewrites.mixed_to_string.contains(&key) {
            self.expr_body(expr)?;
            let span = expr.span().clone();
            let inner = std::mem::replace(expr, Expr::Literal(Literal::Null, span.clone()));
            *expr = Expr::StaticCall { class: "Convert".into(), method: "mixedToStr".into(), args: vec![inner], span, type_args: Vec::new() };
            return Ok(());
        }
        // `mixed` attendu comme un autre type concret : `Convert::mixedAs(expr,
        // "type")`, qui vérifie le type à l'exécution.
        if let Some(descriptor) = self.rewrites.mixed_checks.get(&key) {
            self.expr_body(expr)?;
            let span = expr.span().clone();
            let inner = std::mem::replace(expr, Expr::Literal(Literal::Null, span.clone()));
            let ty = Expr::Literal(Literal::String(descriptor.clone()), span.clone());
            *expr = Expr::StaticCall { class: "Convert".into(), method: "mixedAs".into(), args: vec![inner, ty], span, type_args: Vec::new() };
            return Ok(());
        }
        self.expr_body(expr)
    }

    fn expr_body(&self, expr: &mut Expr) -> Result<(), NamedArgError> {
        // Appel statique sur un `generic` : arguments de type retenus par la
        // sema (voir `crate::sema::generic_static`).
        if let Expr::StaticCall { span, type_args, .. } = expr {
            if let Some(resolved) = self.rewrites.generic_static.get(&site_key(span)) {
                *type_args = resolved.clone();
            }
        }
        if let Expr::Call { span, .. } | Expr::StaticCall { span, .. } = expr {
            if let Some(replacement) = self.rewrites.calls.get(&site_key(span)) {
                *expr = replacement.clone();
            }
        }
        // `truc.A` sur une map : `truc["A"]` (voir `crate::sema::map_dot`).
        if let Expr::Field { object, field, span } = expr {
            if self.rewrites.map_dots.contains(&site_key(span)) {
                let index = Expr::Literal(Literal::String(std::mem::take(field)), span.clone());
                let object = std::mem::replace(object, Box::new(Expr::Literal(Literal::Null, span.clone())));
                *expr = Expr::Index { object, index: Box::new(index), span: span.clone() };
            }
        }
        // `x[k]`/`x.k` lu sur un `mixed` : `Convert::mixedGet(x, k)`.
        if self.rewrites.mixed_reads.contains(&site_key(expr.span())) {
            let span = expr.span().clone();
            if let Some((object, key)) = take_access(expr) {
                *expr = Expr::StaticCall { class: "Convert".into(), method: "mixedGet".into(), args: vec![object, key], span, type_args: Vec::new() };
            }
        }
        // `x -= e` : `String::replace(x, e, "")` sur une string, `x - e` sinon.
        if let Expr::Binary { op: op @ BinOp::Remove, left, right, span } = expr {
            if self.rewrites.string_removals.contains(&site_key(span)) {
                let args = vec![(**left).clone(), (**right).clone(), Expr::Literal(Literal::String(String::new()), span.clone())];
                *expr = Expr::StaticCall { class: "String".into(), method: "replace".into(), args, span: span.clone(), type_args: Vec::new() };
            } else {
                *op = BinOp::Sub;
            }
        }
        match expr {
            Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => Ok(()),
            Expr::Call { callee, args, .. } => {
                self.expr(callee)?;
                self.args(args)
            }
            Expr::StaticCall { args, .. } | Expr::New { args, .. } => self.args(args),
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

/// Receveur et clé d'un accès `x[k]` ou `x.k` (clé `"k"`), retirés de
/// `expr` ; `None` pour toute autre expression.
fn take_access(expr: &mut Expr) -> Option<(Expr, Expr)> {
    let placeholder = Expr::Literal(Literal::Null, expr.span().clone());
    match std::mem::replace(expr, placeholder) {
        Expr::Index { object, index, .. } => Some((*object, *index)),
        Expr::Field { object, field, span } => Some((*object, Expr::Literal(Literal::String(field), span))),
        other => {
            *expr = other;
            None
        }
    }
}

fn unresolved_message(name: &str) -> String {
    format!("named argument '{}' could not be resolved by semantic analysis — pass the arguments positionally", name)
}

/// Ajoute `import ocara.<name>` s'il manque — le codegen ne déclare les
/// fonctions runtime d'un builtin que si son module est importé.
fn ensure_builtin_import(program: &mut Program, name: &str) {
    let imported = program.imports.iter()
        .any(|imp| imp.path.first().is_some_and(|s| s == "ocara") && imp.path.last().is_some_and(|s| s == name));
    if !imported {
        program.imports.push(ImportDecl {
            path:      vec!["ocara".to_string(), name.to_string()],
            file_path: None,
            alias:     None,
            span:      Span::new(0, 0),
        });
    }
}
