/// Tests unitaires — binding `on e is X` typé par la classe du filtre : une
/// méthode inexistante ou un champ appelé comme une méthode sur `e` sont
/// signalés — voir docs/roadmap.d/sema-exception-unknown-method-call.md.
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn check(src: &str) -> Vec<SemaError> {
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

fn in_handler(body: &str) -> String {
    format!(r#"
        import ocara.Exception
        function main(): int {{
            try {{
                raise use Exception("boom", 3)
            }} on e is Exception {{
                {}
            }}
            return 0
        }}
    "#, body)
}

#[test]
fn field_called_as_method_is_reported() {
    let errors = check(&in_handler(r#"var m:string = e.message()"#));
    assert!(errors.iter().any(|e| matches!(e, SemaError::FieldCalledAsMethod { field, .. } if field == "message")), "{:?}", errors);
}

#[test]
fn unknown_method_is_reported() {
    let errors = check(&in_handler(r#"var m:mixed = e.nothing()"#));
    assert!(errors.iter().any(|e| matches!(e, SemaError::FieldNotFound { field, .. } if field == "nothing")), "{:?}", errors);
}

#[test]
fn fields_stay_accessible() {
    let errors = check(&in_handler(r#"var m:string = e.message + " " + e.source
                var c:int = e.code"#));
    assert!(errors.is_empty(), "{:?}", errors);
}

#[test]
fn catch_all_binding_stays_permissive() {
    let src = r#"
        import ocara.Exception
        function main(): int {
            try {
                raise use Exception("boom", 3)
            } on e {
                var m:mixed = e.message
            }
            return 0
        }
    "#;
    assert!(check(src).is_empty());
}
