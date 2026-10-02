/// Tests unitaires — corps des `generic` et des `module` vérifiés par la
/// sema (avec `T` permissif pour un generic) — voir
/// docs/roadmap.d/sema-generic-bodies-unchecked.md.
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::{SemaError, SemaWarning};
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn check(src: &str) -> (Vec<SemaError>, Vec<SemaWarning>) {
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.modules { symbols.register_module(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    (checker.errors, checker.warnings)
}

#[test]
fn errors_in_an_uninstantiated_generic_body_are_reported() {
    let src = r#"
        generic Box<T> {
            public method broken(): int {
                var n:int = "pas un entier"
                return inconnu
            }
        }
        function main(): int { return 0 }
    "#;
    let (errors, _) = check(src);
    assert!(errors.iter().any(|e| matches!(e, SemaError::TypeMismatch { .. })), "{:?}", errors);
    assert!(errors.iter().any(|e| matches!(e, SemaError::UndefinedSymbol { name, .. } if name == "inconnu")), "{:?}", errors);
}

#[test]
fn type_parameter_values_are_permissive_and_raise_no_mixed_diagnostic() {
    let src = r#"
        generic Box<T> {
            private property item:T
            init(item:T) { self.item = item }
            public method get(): T {
                var copy:T = self.item
                return copy
            }
            public method describe(): string {
                return self.get()
            }
        }
        function main(): int { return 0 }
    "#;
    let (errors, warnings) = check(src);
    assert!(errors.is_empty(), "{:?}", errors);
    assert!(!warnings.iter().any(|w| matches!(w, SemaWarning::MixedLocalVariable { .. })), "{:?}", warnings);
}

#[test]
fn self_method_calls_in_a_generic_are_resolved() {
    let src = r#"
        generic Box<T> {
            public method put(item:T, times:int = 1): void { }
            public method twice(item:T): void { self.put(times: 2, item: item) }
            public method wrong(): void { self.nope() }
        }
        function main(): int { return 0 }
    "#;
    let (errors, _) = check(src);
    assert_eq!(errors.len(), 1, "{:?}", errors);
    assert!(matches!(&errors[0], SemaError::FieldNotFound { field, .. } if field == "nope"));
}

#[test]
fn errors_in_a_module_body_are_reported() {
    let src = r#"
        module Tagged {
            public method touch(): void {
                var n:int = "x"
            }
        }
        class User modules Tagged { }
        function main(): int { return 0 }
    "#;
    let (errors, _) = check(src);
    assert!(errors.iter().any(|e| matches!(e, SemaError::TypeMismatch { .. })), "{:?}", errors);
}
