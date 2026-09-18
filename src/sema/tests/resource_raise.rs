/// Tests unitaires — W04, ressource scoped/consumed ouverte au moment d'un
/// raise non rattrapé localement (docs/roadmap.d/exceptions-setjmp-longjmp-dette.md).
///
/// Graphes de contrôle synthétiques minimaux, construits directement en AST
/// (pas de parsing) — un par cas décrit dans la doc de
/// `crate::sema::resource_raise`.

use std::collections::HashSet;
use crate::sema::resource_raise::check_program;
use crate::sema::error::SemaWarning;
use crate::parsing::ast::*;
use crate::parsing::token::Span;

fn span() -> Span {
    Span::new(0, 0)
}

fn block(stmts: Vec<Stmt>) -> Block {
    Block { stmts, span: span() }
}

fn scoped_mutex(name: &str) -> Stmt {
    Stmt::Var {
        name: name.to_string(),
        ty: Type::Named("Mutex".into()),
        value: Expr::New { class: "Mutex".into(), type_args: vec![], args: vec![], span: span() },
        mutable: true,
        kind: VarKind::Scoped,
        span: span(),
    }
}

fn raise_stmt() -> Stmt {
    Stmt::Raise { value: Expr::Literal(Literal::String("boom".into()), span()), span: span() }
}

fn finalize_call(name: &str, method: &str) -> Stmt {
    Stmt::Expr(Expr::Call {
        callee: Box::new(Expr::Field { object: Box::new(Expr::Ident(name.to_string(), span())), field: method.to_string(), span: span() }),
        args: vec![],
        span: span(),
    })
}

fn program_with_main_body(body: Block) -> Program {
    let mut p = Program::new();
    p.functions.push(FuncDecl {
        name: "main".into(),
        params: vec![],
        ret_ty: Type::Void,
        body,
        is_async: false,
        span: span(),
    });
    p
}

fn find_leak_warning<'a>(warnings: &'a [SemaWarning], var_name: &str) -> Option<&'a SemaWarning> {
    warnings.iter().find(|w| matches!(w, SemaWarning::ScopedResourceRaiseLeak { name, .. } if name == var_name))
}

/// Cas direct : `scoped m:Mutex`, puis un `raise` plus loin dans le MÊME
/// bloc, sans aucun `try` — doit être signalé.
#[test]
fn direct_raise_after_resource_in_same_block_warns() {
    let program = program_with_main_body(block(vec![
        scoped_mutex("m"),
        raise_stmt(),
    ]));
    let warnings = check_program(&program, &HashSet::new());
    assert!(find_leak_warning(&warnings, "m").is_some());
}

/// La ressource est finalisée (`.destroy()`) en ligne droite AVANT le
/// `raise`, dans le même bloc — ne doit PAS être signalée.
#[test]
fn finalized_before_raise_does_not_warn() {
    let program = program_with_main_body(block(vec![
        scoped_mutex("m"),
        finalize_call("m", "destroy"),
        raise_stmt(),
    ]));
    let warnings = check_program(&program, &HashSet::new());
    assert!(find_leak_warning(&warnings, "m").is_none());
}

/// Le `raise` est à l'intérieur d'un `try` propre à CE bloc — assumé
/// rattrapé localement, ne doit PAS être signalé.
#[test]
fn raise_inside_local_try_does_not_warn() {
    let program = program_with_main_body(block(vec![
        scoped_mutex("m"),
        Stmt::Try {
            body: block(vec![raise_stmt()]),
            handlers: vec![OnClause { binding: "e".into(), class_filter: None, body: block(vec![]), span: span() }],
            span: span(),
        },
    ]));
    let warnings = check_program(&program, &HashSet::new());
    assert!(find_leak_warning(&warnings, "m").is_none());
}

/// Le `raise` est atteignable via un `if` imbriqué SANS son propre `try` —
/// rien ne l'arrête, doit être signalé (même bloc englobant concerné).
#[test]
fn raise_reachable_through_nested_if_without_try_warns() {
    let program = program_with_main_body(block(vec![
        scoped_mutex("m"),
        Stmt::If {
            condition: Expr::Literal(Literal::Bool(true), span()),
            then_block: block(vec![raise_stmt()]),
            elseif: vec![],
            else_block: None,
            span: span(),
        },
    ]));
    let warnings = check_program(&program, &HashSet::new());
    assert!(find_leak_warning(&warnings, "m").is_some());
}

/// Le `raise` est dans le handler `on` d'un `try` (pas son corps protégé) —
/// plus rien ne le protège, doit être signalé.
#[test]
fn raise_inside_on_handler_warns() {
    let program = program_with_main_body(block(vec![
        scoped_mutex("m"),
        Stmt::Try {
            body: block(vec![]),
            handlers: vec![OnClause { binding: "e".into(), class_filter: None, body: block(vec![raise_stmt()]), span: span() }],
            span: span(),
        },
    ]));
    let warnings = check_program(&program, &HashSet::new());
    assert!(find_leak_warning(&warnings, "m").is_some());
}

/// Aucune ressource déclarée du tout — un `raise` seul ne doit jamais être
/// signalé (rien à protéger).
#[test]
fn raise_without_any_resource_does_not_warn() {
    let program = program_with_main_body(block(vec![raise_stmt()]));
    let warnings = check_program(&program, &HashSet::new());
    assert!(warnings.is_empty());
}

/// Un `var` (pas `scoped`/`consumed`) n'est jamais concerné par ce
/// diagnostic — c'est le rôle d'un diagnostic distinct (E28,
/// `UnclosedResourceVar`), pas de celui-ci.
#[test]
fn plain_var_resource_is_not_checked_by_this_warning() {
    let program = program_with_main_body(block(vec![
        Stmt::Var {
            name: "m".into(),
            ty: Type::Named("Mutex".into()),
            value: Expr::New { class: "Mutex".into(), type_args: vec![], args: vec![], span: span() },
            mutable: true,
            kind: VarKind::Var,
            span: span(),
        },
        raise_stmt(),
    ]));
    let warnings = check_program(&program, &HashSet::new());
    assert!(find_leak_warning(&warnings, "m").is_none());
}
