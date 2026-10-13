/// Tests unitaires — constantes globales : toute expression est acceptée
/// (évaluée une seule fois en tête de `main`), compatible avec le type
/// déclaré.
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn errors(src: &str) -> Vec<SemaError> {
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.consts { symbols.register_const(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

#[test]
fn literal_and_folded_values_are_accepted() {
    let src = "const A:int = -60 * 1000\nconst B:string = \"a\" + \"b\"\nfunction main(): int { return A }";
    assert!(errors(src).is_empty());
}

#[test]
fn call_is_accepted() {
    let src = "function f(): int { return 1 }\nconst G:int = f()\nfunction main(): int { return G }";
    assert!(errors(src).is_empty());
}

#[test]
fn value_must_match_declared_type() {
    let src = "function f(): string { return \"x\" }\nconst G:int = f()\nfunction main(): int { return G }";
    let found: Vec<String> = errors(src).iter().filter_map(|e| match e {
        SemaError::TypeMismatch { expected, found, .. } => Some(format!("{}/{}", expected, found)),
        _ => None,
    }).collect();
    assert_eq!(found, vec!["int/string".to_string()]);
}
