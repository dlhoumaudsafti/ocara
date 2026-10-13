/// Tests unitaires — appel à une fonction ou une classe inconnue (E01) —
/// voir docs/roadmap.d/sema-appel-symbole-inconnu-accepte.md.
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn errors(main_body: &str) -> Vec<SemaError> {
    let src = format!(r#"
        class K {{
            public static method label(): string {{ return "k" }}
        }}
        function known(): int {{ return 1 }}
        function main(): int {{
            {}
            return 0
        }}
    "#, main_body);
    let program = parse(&src);
    let mut symbols = SymbolTable::new();
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

fn undefined(errs: &[SemaError]) -> Vec<String> {
    errs.iter().filter_map(|e| match e {
        SemaError::UndefinedSymbol { name, .. } => Some(name.clone()),
        _ => None,
    }).collect()
}

#[test]
fn unknown_free_function_is_rejected() {
    assert_eq!(undefined(&errors("var x:mixed = foo()")), vec!["foo".to_string()]);
}

#[test]
fn unknown_class_static_call_is_rejected() {
    assert_eq!(undefined(&errors("var x:mixed = Nope::label()")), vec!["Nope::label".to_string()]);
}

#[test]
fn known_targets_and_local_function_values_are_accepted() {
    let errs = errors(r#"
        var a:int = known()
        var b:string = K::label()
        var f:Function<int()> = nameless(): int { return 2 }
        var c:int = f()
    "#);
    assert!(undefined(&errs).is_empty(), "{:?}", errs);
}

#[test]
fn class_called_without_use_is_not_callable() {
    let errs = errors("var k:K = K()");
    assert!(errs.iter().any(|e| matches!(e, SemaError::NotCallable { .. })), "{:?}", errs);
}
