//! Tests des clauses `when` (voir when.rs).

use crate::parsing::ast::*;
use crate::parsing::build::{with_config, BuildConfig};
use crate::parsing::lexer::Lexer;
use crate::parsing::parser::Parser;

fn config(os: &str, build: &str) -> BuildConfig {
    let mut c = BuildConfig::for_target(None, build == "release");
    c.values.insert("OS".into(), os.into());
    c
}

fn parse_for(src: &str, cfg: BuildConfig) -> Result<Program, String> {
    let tokens = Lexer::new(src).tokenize().expect("lex error");
    with_config(cfg, || Parser::new(tokens).parse_program()).map_err(|e| e.message)
}

fn returned(f: &FuncDecl) -> String {
    match &f.body.stmts[0] {
        Stmt::Return { value: Some(Expr::Literal(Literal::String(s), _)), .. } => s.clone(),
        other => panic!("retour inattendu : {:?}", other),
    }
}

const HOME: &str = "
when System::OS is 'windows'
function home(): string { return 'win' }
when System::OS is 'linux' or System::OS is 'android'
function home(): string { return 'unix' }
when default
function home(): string { return 'other' }
";

#[test]
fn first_true_clause_wins_then_default() {
    for (os, expected) in [("windows", "win"), ("linux", "unix"), ("android", "unix"), ("macos", "other")] {
        let p = parse_for(HOME, config(os, "debug")).unwrap();
        assert_eq!(p.functions.len(), 1, "{}", os);
        assert_eq!(returned(&p.functions[0]), expected, "{}", os);
    }
}

#[test]
fn stacked_clauses_are_and_and_comparisons_work() {
    let src = "
when System::OS is not 'windows'
when System::BUILD is 'release'
function f(): string { return 'a' }
when System::BUILD is greater or equal 'debug'
function f(): string { return 'b' }
";
    assert_eq!(returned(&parse_for(src, config("linux", "release")).unwrap().functions[0]), "a");
    assert_eq!(returned(&parse_for(src, config("windows", "release")).unwrap().functions[0]), "b");
    assert!(parse_for(src, config("linux", "debug")).unwrap().functions.len() == 1);
}

#[test]
fn absent_variant_removes_declaration() {
    let src = "when System::OS is not 'android'\nimport ocara.Tauri\nimport ocara.IO\n";
    assert_eq!(parse_for(src, config("android", "debug")).unwrap().imports.len(), 1);
    assert_eq!(parse_for(src, config("linux", "debug")).unwrap().imports.len(), 2);
}

#[test]
fn dropped_property_takes_its_initializer() {
    let src = "class C {
    when System::OS is 'linux'
    public property ret:int = 1
    when default
    public property ret:string = \"3\"
}";
    let p = parse_for(src, config("macos", "debug")).unwrap();
    let fields: Vec<&Type> = p.classes[0].members.iter()
        .filter_map(|m| match m { ClassMember::Field { ty, .. } => Some(ty), _ => None })
        .collect();
    assert_eq!(fields, vec![&Type::String]);
    let Some(ClassMember::Constructor { body, .. }) = p.classes[0].members.iter().find(|m| matches!(m, ClassMember::Constructor { .. })) else {
        panic!("init implicite attendu");
    };
    assert_eq!(body.stmts.len(), 1);
}

#[test]
fn consecutive_conditioned_statements_form_a_group() {
    let src = "function f(): string {
    var a:int = 1
    when System::OS is 'linux'
    return 'l'
    when default
    return 'd'
}";
    let linux = parse_for(src, config("linux", "debug")).unwrap();
    assert_eq!(linux.functions[0].body.stmts.len(), 2);
    let other = parse_for(src, config("windows", "debug")).unwrap();
    assert_eq!(other.functions[0].body.stmts.len(), 2);
    assert!(matches!(&other.functions[0].body.stmts[1], Stmt::Return { value: Some(Expr::Literal(Literal::String(s), _)), .. } if s == "d"));
}

#[test]
fn two_fallbacks_are_an_error() {
    let src = "function f(): int { return 1 }\nwhen default\nfunction f(): int { return 2 }\n";
    assert!(parse_for(src, config("linux", "debug")).unwrap_err().contains("sinon"));
}

#[test]
fn unknown_constant_is_an_error() {
    let src = "when System::FOO is 'x'\nfunction f(): int { return 1 }\n";
    assert!(parse_for(src, config("linux", "debug")).unwrap_err().contains("System::FOO"));
}

#[test]
fn keep_all_keeps_every_variant() {
    assert_eq!(parse_for(HOME, BuildConfig::keep_all()).unwrap().functions.len(), 3);
}

#[test]
fn when_default_closes_a_statement_group() {
    let src = "function f(): void {
    when System::BUILD is 'debug'
    g(1)
    when default
    g(2)
    when System::OS is 'linux'
    g(3)
}";
    assert_eq!(parse_for(src, config("linux", "debug")).unwrap().functions[0].body.stmts.len(), 2);
    assert_eq!(parse_for(src, config("linux", "release")).unwrap().functions[0].body.stmts.len(), 2);
    assert_eq!(parse_for(src, config("windows", "release")).unwrap().functions[0].body.stmts.len(), 1);
}
