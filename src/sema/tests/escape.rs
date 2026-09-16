/// Tests unitaires — analyse d'échappement interprocédurale
/// (docs/roadmap.d/qualite-tests-unitaires-critiques.md, point 4).
///
/// Graphes d'appel synthétiques minimaux, construits directement en AST
/// (pas de parsing) — un par cas documenté dans l'en-tête de
/// `src/sema/escape.rs` : un paramètre retenu (affecté à un champ), un
/// paramètre seulement lu (receveur d'un appel de méthode), et l'asymétrie
/// stricte/non-stricte sur un callee non résolu (builtin).

use std::collections::HashMap;
use crate::sema::escape::{compute_escaping_params, var_never_escapes};
use crate::parsing::ast::*;
use crate::parsing::token::Span;

fn span() -> Span {
    Span::new(0, 0)
}

fn ident(name: &str) -> Expr {
    Expr::Ident(name.to_string(), span())
}

fn param(name: &str) -> Param {
    Param { name: name.to_string(), ty: Type::Int, default_value: None, is_variadic: false, span: span() }
}

fn block(stmts: Vec<Stmt>) -> Block {
    Block { stmts, span: span() }
}

/// Constructeur `Box::init(a)` dont le corps fait `self.data = a` — le
/// scénario exact documenté comme diagnostic E26 dans docs/EBNF.md §9.2.1.
/// Le paramètre affecté à un champ doit être détecté comme échappant.
#[test]
fn param_assigned_to_field_escapes() {
    let mut program = Program::new();
    program.classes.push(ClassDecl {
        name: "Box".into(),
        extends: None,
        modules: vec![],
        implements: vec![],
        members: vec![ClassMember::Constructor {
            params: vec![param("a")],
            body: block(vec![Stmt::Assign {
                target: Expr::Field { object: Box::new(Expr::SelfExpr(span())), field: "data".into(), span: span() },
                value: ident("a"),
                span: span(),
            }]),
            span: span(),
        }],
        span: span(),
    });

    let result = compute_escaping_params(&program);
    assert_eq!(result.get("Box::init"), Some(&vec![true]));
}

/// `read(p) { p.doSomething() }` — `p` n'est reçu que comme RÉCEPTEUR d'un
/// appel de méthode, jamais passé en argument ni stocké nulle part. Ne doit
/// pas être marqué échappant : muter/lire un paramètre ne le fait jamais
/// s'échapper, seul le fait de le CÉDER (retour, champ, argument retenu...)
/// compte (voir docs/EBNF.md §9.1, même principe pour `var`).
#[test]
fn param_used_only_as_method_receiver_does_not_escape() {
    let mut program = Program::new();
    program.functions.push(FuncDecl {
        name: "read".into(),
        params: vec![param("p")],
        ret_ty: Type::Void,
        body: block(vec![Stmt::Expr(Expr::Call {
            callee: Box::new(Expr::Field { object: Box::new(ident("p")), field: "doSomething".into(), span: span() }),
            args: vec![],
            span: span(),
        })]),
        is_async: false,
        span: span(),
    });

    let result = compute_escaping_params(&program);
    assert_eq!(result.get("read"), Some(&vec![false]));
}

/// `pushInto(arr, x) { Array::push(arr, x) }` — `Array` n'est pas une classe
/// utilisateur (absente de `class_members`), le callee reste donc non résolu.
/// `compute_escaping_params` (consommé par le diagnostic E26, MODE NON
/// STRICT — voir l'en-tête de doc de `escape.rs`) ignore alors purement et
/// simplement cet appel : ni `arr` ni `x` ne sont marqués échappants, même si
/// `Array::push` retient réellement son 2ᵉ argument en réalité. Imprécision
/// ASSUMÉE et documentée (rater un échappement n'est pas pire qu'avant, où
/// rien n'était vérifié) — ce test fige ce choix pour qu'il ne soit jamais
/// changé par accident.
#[test]
fn argument_to_unresolved_builtin_is_not_flagged_in_non_strict_mode() {
    let mut program = Program::new();
    program.functions.push(FuncDecl {
        name: "pushInto".into(),
        params: vec![param("arr"), param("x")],
        ret_ty: Type::Void,
        body: block(vec![Stmt::Expr(Expr::StaticCall {
            class: "Array".into(),
            method: "push".into(),
            args: vec![ident("arr"), ident("x")],
            span: span(),
        })]),
        is_async: false,
        span: span(),
    });

    let result = compute_escaping_params(&program);
    assert_eq!(result.get("pushInto"), Some(&vec![false, false]));
}

/// Même corps que le cas précédent (`Array::push(arr, x)`), mais via
/// `var_never_escapes` — MODE STRICT, utilisé pour la libération automatique
/// d'un `var` (voir sa doc : un callee non résolu doit être traité comme
/// retenant TOUS ses arguments, sinon UAF confirmé par reproduction sur
/// `Array::push`). Contrairement au test précédent, `x` DOIT être détecté
/// comme échappant ici — c'est exactement l'asymétrie stricte/non-stricte
/// que ce module documente comme volontaire, pas un bug si les deux tests
/// donnent des réponses différentes sur le même appel.
#[test]
fn var_never_escapes_is_strict_and_catches_what_compute_escaping_params_misses() {
    let stmts = block(vec![
        Stmt::Var { name: "x".into(), ty: Type::Int, value: Expr::Literal(Literal::Int(5), span()), mutable: true, kind: VarKind::Scoped, span: span() },
        Stmt::Expr(Expr::StaticCall { class: "Array".into(), method: "push".into(), args: vec![ident("arr"), ident("x")], span: span() }),
    ]);

    let class_members: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    let known = HashMap::new();
    assert!(!var_never_escapes(&class_members, "x", &stmts, 0, None, &known));
}

/// `var_never_escapes` sur un paramètre seulement lu (méthode receveur,
/// comme le 2ᵉ test ci-dessus) : doit rester `true` (libérable) — vérifie
/// que le mode strict ne devient pas trop conservateur au point de bloquer
/// la libération automatique d'un `var` dans le cas le plus courant.
#[test]
fn var_never_escapes_true_when_only_read_via_method_call() {
    let stmts = block(vec![
        Stmt::Var { name: "x".into(), ty: Type::Int, value: Expr::Literal(Literal::Int(5), span()), mutable: true, kind: VarKind::Scoped, span: span() },
        Stmt::Expr(Expr::Call {
            callee: Box::new(Expr::Field { object: Box::new(ident("x")), field: "len".into(), span: span() }),
            args: vec![],
            span: span(),
        }),
    ]);

    let class_members: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    let known = HashMap::new();
    assert!(var_never_escapes(&class_members, "x", &stmts, 0, None, &known));
}
