/// Tests unitaires — E61, valeur d'une constante globale évaluable à la
/// compilation (réévaluée à l'entrée de chaque fonction : un appel y
/// bouclait jusqu'au débordement de pile).
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

/// Noms des constantes globales signalées par E61.
fn non_constant_globals(src: &str) -> Vec<String> {
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors.iter()
        .filter_map(|e| match e {
            SemaError::GlobalConstNotConstant { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn literal_and_folded_values_are_accepted() {
    let src = "const A:int = -60 * 1000\nconst B:string = \"a\" + \"b\"\nfunction main(): int { return A }";
    assert!(non_constant_globals(src).is_empty());
}

#[test]
fn call_is_rejected() {
    let src = "function f(): int { return 1 }\nconst G:int = f()\nfunction main(): int { return G }";
    assert_eq!(non_constant_globals(src), vec!["G".to_string()]);
}
