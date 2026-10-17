/// Tests unitaires — W04, `raise` non rattrapé localement qui saute la
/// finalisation d'un `Thread` ou d'une ressource native `var`
/// (voir `crate::sema::resource_raise`).
///
/// Graphes de contrôle synthétiques minimaux, construits directement en AST
/// (pas de parsing) — un par cas décrit dans la doc du module.

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

fn decl(name: &str, class: &str, kind: VarKind) -> Stmt {
    Stmt::Var {
        name: name.to_string(),
        ty: Type::Named(class.into()),
        value: Expr::New { class: class.into(), type_args: vec![], args: vec![], span: span() },
        mutable: true,
        kind,
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

fn warns_for(body: Vec<Stmt>, var_name: &str) -> bool {
    let mut p = Program::new();
    p.functions.push(FuncDecl {
        name: "main".into(),
        params: vec![],
        ret_ty: Type::Void,
        body: block(body),
        is_async: false,
        span: span(),
    });
    check_program(&p).iter().any(|w| matches!(w, SemaWarning::RaiseSkipsFinalization { name, .. } if name == var_name))
}

/// `scoped t:Thread` puis un `raise` dans le même bloc : le `.join()`
/// obligatoire est sauté — signalé.
#[test]
fn scoped_thread_then_raise_warns() {
    assert!(warns_for(vec![decl("t", "Thread", VarKind::Scoped), raise_stmt(), finalize_call("t", "join")], "t"));
}

/// Une ressource native `scoped` est fermée par le déroulement du `raise`
/// (`__rc_resource_push`) — jamais signalée.
#[test]
fn scoped_native_resource_is_closed_by_unwind_and_not_warned() {
    assert!(!warns_for(vec![decl("db", "SQLite", VarKind::Scoped), raise_stmt()], "db"));
    assert!(!warns_for(vec![decl("m", "Mutex", VarKind::Consumed), raise_stmt()], "m"));
}

/// `var db:SQLite` puis `raise` puis `db.close()` : le `raise` saute la
/// fermeture manuelle — signalé.
#[test]
fn var_resource_closed_after_raise_warns() {
    assert!(warns_for(vec![decl("db", "SQLite", VarKind::Var), raise_stmt(), finalize_call("db", "close")], "db"));
}

/// `var` jamais fermée dans ce bloc (ressource partagée ailleurs) : le
/// `raise` ne saute rien — pas signalé.
#[test]
fn var_resource_never_closed_here_does_not_warn() {
    assert!(!warns_for(vec![decl("m", "Mutex", VarKind::Var), raise_stmt()], "m"));
}

/// Finalisation en ligne droite AVANT le `raise` — pas signalé.
#[test]
fn finalized_before_raise_does_not_warn() {
    assert!(!warns_for(vec![decl("t", "Thread", VarKind::Scoped), finalize_call("t", "join"), raise_stmt()], "t"));
}

/// `raise` dans le corps d'un `try` local — assumé rattrapé, pas signalé.
#[test]
fn raise_inside_local_try_does_not_warn() {
    assert!(!warns_for(vec![
        decl("t", "Thread", VarKind::Scoped),
        Stmt::Try {
            body: block(vec![raise_stmt()]),
            handlers: vec![OnClause { binding: "e".into(), class_filter: None, body: block(vec![]), span: span() }],
            span: span(),
        },
        finalize_call("t", "join"),
    ], "t"));
}

/// `raise` atteignable via un `if` imbriqué sans `try` propre — signalé.
#[test]
fn raise_reachable_through_nested_if_without_try_warns() {
    assert!(warns_for(vec![
        decl("db", "SQLite", VarKind::Var),
        Stmt::If {
            condition: Expr::Literal(Literal::Bool(true), span()),
            then_block: block(vec![raise_stmt()]),
            elseif: vec![],
            else_block: None,
            span: span(),
        },
        finalize_call("db", "close"),
    ], "db"));
}

/// `raise` dans le handler `on` d'un `try` — plus rien ne le protège, signalé.
#[test]
fn raise_inside_on_handler_warns() {
    assert!(warns_for(vec![
        decl("t", "Thread", VarKind::Scoped),
        Stmt::Try {
            body: block(vec![]),
            handlers: vec![OnClause { binding: "e".into(), class_filter: None, body: block(vec![raise_stmt()]), span: span() }],
            span: span(),
        },
        finalize_call("t", "detach"),
    ], "t"));
}
