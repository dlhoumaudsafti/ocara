/// Tests unitaires — `ui.bind(racine, objet)` (src/sema/ui_bind.rs) :
/// accesseur généré, chemins du `.oc.ui` vérifiés contre la classe.
use crate::core::named_args::rewrite_program;
use crate::parsing::ast::{Expr, Literal, Program, Stmt};
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

const CLASSES: &str = r#"
    class Address {
        public property city:string = "Paris"
    }
    class Person {
        public property name:string = "Ana"
        public property age:int = 30
        public property ratio:float = 0.5
        public property address:Address|null = null
        public property tags:array<string> = []
        private property secret:string = "x"
    }
"#;

fn check(body: &str) -> (Vec<SemaError>, Program) {
    let src = format!("import ocara.UI\n{CLASSES}\nfunction main(ui:UI): int {{\n    {body}\n    return 0\n}}\n");
    let tokens = Lexer::new(&src).tokenize().expect("lex ok");
    let mut program = Parser::new(tokens).parse_program().expect("parse ok");
    let mut symbols = SymbolTable::new();
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    let errors: Vec<SemaError> = std::mem::take(&mut checker.errors);
    let rewrites = std::mem::take(&mut checker.rewrites);
    if errors.is_empty() {
        rewrite_program(&mut program, &rewrites).expect("rewrite ok");
    }
    (errors, program)
}

fn bind_errors(body: &str) -> Vec<String> {
    check(body).0.into_iter().filter(|e| matches!(e, SemaError::UiBind { .. })).map(|e| e.message()).collect()
}

/// Table `roots.user` d'une interface dont le nœud `n` lie `path` (nature `kind`).
fn spec(path: &str, kind: &str) -> String {
    format!(r#"'{{"paths":["{path}"],"nodes":[{{"id":"n","prop":"value","path":0,"kind":"{kind}"}}]}}'"#)
}

#[test]
fn call_gets_the_accessor_and_its_paths() {
    let (errors, program) = check(r#"ui.bind("user", use Person())"#);
    assert!(errors.is_empty(), "{:?}", errors);
    let Stmt::Expr(Expr::Call { args, .. }) = &program.functions[0].body.stmts[0] else { panic!("appel attendu") };
    assert_eq!(args.len(), 4);
    let Expr::Literal(Literal::String(paths), _) = &args[2] else { panic!("chemins attendus") };
    let paths: serde_json::Value = serde_json::from_str(paths).unwrap();
    let names: Vec<&str> = paths.as_array().unwrap().iter().map(|p| p["path"].as_str().unwrap()).collect();
    assert_eq!(names, ["address.city", "age", "name", "ratio"]);
    assert_eq!(paths[1]["kinds"], serde_json::json!(["text", "number"]));
    assert!(matches!(args[3], Expr::Nameless { .. }));
}

#[test]
fn paths_of_a_known_interface_are_checked() {
    let cases = [
        ("name", "text", None),
        ("age", "number", None),
        ("address.city", "text", None),
        ("age", "bool", Some("does not fit")),
        ("email", "text", Some("has no field 'email'")),
        ("secret", "text", Some("not accessible")),
        ("tags", "text", Some("only string, int, float, bool, mixed and object")),
        ("address", "text", Some("is an object")),
        ("name.first", "text", Some("'name' has no fields")),
    ];
    for (path, kind, expected) in cases {
        let errors = bind_errors(&format!("ui.bind(\"user\", use Person(), {})", spec(path, kind)));
        match expected {
            None => assert!(errors.is_empty(), "{path} : {errors:?}"),
            Some(part) => assert!(errors.len() == 1 && errors[0].contains(part), "{path} : {errors:?}"),
        }
    }
}

#[test]
fn the_object_must_be_an_instance_of_a_class() {
    let errors = bind_errors(r#"ui.bind("user", 3)"#);
    assert!(errors.len() == 1 && errors[0].contains("an object of a class is expected"), "{errors:?}");
}
