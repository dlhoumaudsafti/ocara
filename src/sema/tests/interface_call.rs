/// Tests unitaires — appel de méthode sur une valeur typée par une interface
/// (`s.speak()` avec `s:Speaker`) : arité et type de retour vérifiés comme
/// pour une classe — voir docs/roadmap.d/sema-appel-via-interface-non-verifie.md.
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn check_errors(body: &str) -> Vec<SemaError> {
    let src = format!("interface Speaker {{\n    public method speak(): string\n}}\n\nclass Animal implements Speaker {{\n    public method speak(): string {{\n        return \"...\"\n    }}\n}}\n\nfunction main(): int {{\n    var s:Speaker = use Animal()\n{}\n    return 0\n}}\n", body);
    let program = parse(&src);
    let mut symbols = SymbolTable::new();
    for decl in &program.interfaces { symbols.register_interface(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

#[test]
fn interface_call_return_type_is_checked() {
    let errors = check_errors("    var n:int = s.speak()\n    IO::writeln(n)");
    assert!(errors.iter().any(|e| matches!(e, SemaError::TypeMismatch { .. })), "{:?}", errors);
}

#[test]
fn interface_call_arity_is_checked() {
    let errors = check_errors("    var t:string = s.speak(1)\n    IO::writeln(t)");
    assert!(errors.iter().any(|e| matches!(e, SemaError::WrongArgCount { .. })), "{:?}", errors);
}

#[test]
fn interface_call_well_typed_is_accepted() {
    let errors = check_errors("    var t:string = s.speak()\n    IO::writeln(t)");
    assert!(!errors.iter().any(|e| matches!(e, SemaError::TypeMismatch { .. } | SemaError::WrongArgCount { .. })), "{:?}", errors);
}
