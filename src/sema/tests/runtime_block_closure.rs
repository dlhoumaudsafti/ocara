/// Tests unitaires — closure déclarée dans un bloc runtime : son corps est une
/// fonction ordinaire (`return` y quitte la closure, `result` y est interdit).
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn check(src: &str) -> Vec<SemaError> {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    let program = Parser::new(tokens).parse_program().expect("parse ok");
    let symbols = SymbolTable::new();
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    std::mem::take(&mut checker.errors)
}

#[test]
fn return_inside_closure_of_runtime_block_is_allowed() {
    let errors = check("main {\n var f:Function<int()> = nameless(): int {\n return 41\n }\n f()\n }");
    assert!(!errors.iter().any(|e| matches!(e, SemaError::ReturnInsideRuntimeBlock { .. })), "{:?}", errors);
}

#[test]
fn return_directly_in_runtime_block_is_rejected() {
    let errors = check("main {\n return 1\n }");
    assert!(errors.iter().any(|e| matches!(e, SemaError::ReturnInsideRuntimeBlock { .. })), "{:?}", errors);
}

#[test]
fn result_inside_closure_of_runtime_block_is_rejected() {
    let errors = check("main {\n var f:Function<void()> = nameless(): void {\n result ERROR\n }\n f()\n }");
    assert!(errors.iter().any(|e| matches!(e, SemaError::ResultOutsideRuntimeBlock { .. })), "{:?}", errors);
}
