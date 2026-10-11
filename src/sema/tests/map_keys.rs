/// Tests unitaires — type par clé d'une map initialisée par un littéral
/// (voir src/sema/map_keys.rs, EBNF §7.3).
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn check_errors(body: &str) -> Vec<SemaError> {
    let src = format!("class Car {{\n    public property brand:string = \"x\"\n}}\n\nfunction main(): int {{\n    var car:Car = use Car()\n{}\n    return 0\n}}\n", body);
    let tokens = Lexer::new(&src).tokenize().expect("lex ok");
    let program: Program = Parser::new(tokens).parse_program().expect("parse ok");
    let mut symbols = SymbolTable::new();
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

fn has(errors: &[SemaError], f: impl Fn(&SemaError) -> bool) -> bool {
    errors.iter().any(f)
}

const DETAILS: &str = "    const d:map<string, Car|string> = {\"car\": car, \"html\": \"\"}\n";

#[test]
fn literal_key_keeps_its_value_type() {
    let errors = check_errors(&format!("{}    d['html'] += \"<li>\"\n    var c:Car = d['car']\n    IO::writeln(d['html'] + c.brand)", DETAILS));
    assert!(!has(&errors, |e| matches!(e, SemaError::TypeMismatch { .. })), "{:?}", errors);
    assert!(!errors.iter().any(|e| e.message().contains("cannot concatenate")), "{:?}", errors);
}

#[test]
fn assignment_to_typed_key_is_checked() {
    let errors = check_errors(&format!("{}    d['html'] = car", DETAILS));
    assert!(has(&errors, |e| matches!(e, SemaError::TypeMismatch { .. })), "{:?}", errors);
}

#[test]
fn unknown_key_falls_back_to_declared_value_type() {
    let errors = check_errors(&format!("{}    var x:string = d['autre'] + \"!\"\n    IO::writeln(x)", DETAILS));
    assert!(errors.iter().any(|e| e.message().contains("cannot concatenate")), "{:?}", errors);
}
