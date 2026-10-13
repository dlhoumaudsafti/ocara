/// Tests unitaires — types des arguments à l'appel (E62) — voir
/// docs/roadmap.d/sema-types-arguments-appel.md.
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
    errors_in(&format!(r#"
        import ocara.Math
        interface Shape {{
            method area(scale:int): int
        }}
        class P {{
            public property x:int
            init(x:int) {{ self.x = x }}
        }}
        class Q extends P {{
            init(x:int, label:string) {{ parent::init(x) }}
        }}
        class C {{
            public method add(a:int, b:int): int {{ return a + b }}
            public static method sadd(a:int, b:int): int {{ return a + b }}
        }}
        class Sq implements Shape {{
            public method area(scale:int): int {{ return scale }}
        }}
        function add(a:int, b:int): int {{ return a + b }}
        function takeP(p:P): int {{ return p.x }}
        function sum(xs:variadic<int>): int {{ return 0 }}
        function opt(a:int, b:string|null = null): int {{ return a }}
        function main(): int {{
            {}
            return 0
        }}
    "#, main_body))
}

fn errors_in(src: &str) -> Vec<SemaError> {
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.interfaces { symbols.register_interface(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

/// `(appelé, paramètre, attendu, trouvé)` de chaque E62.
fn mismatches(errs: &[SemaError]) -> Vec<(String, String, String, String)> {
    errs.iter().filter_map(|e| match e {
        SemaError::ArgTypeMismatch { callee, param, expected, found, .. } =>
            Some((callee.clone(), param.clone(), expected.clone(), found.clone())),
        _ => None,
    }).collect()
}

fn one(callee: &str, param: &str, expected: &str, found: &str) -> Vec<(String, String, String, String)> {
    vec![(callee.into(), param.into(), expected.into(), found.into())]
}

#[test]
fn free_function_argument_type_is_checked() {
    assert_eq!(mismatches(&errors(r#"add("1", 2)"#)), one("add", "a", "int", "string"));
}

#[test]
fn named_argument_type_is_checked() {
    assert_eq!(mismatches(&errors(r#"add(b: 2, a: "un")"#)), one("add", "a", "int", "string"));
}

#[test]
fn object_parameter_rejects_other_types() {
    assert_eq!(mismatches(&errors(r#"takeP("pas un P")"#)), one("takeP", "p", "P", "string"));
}

#[test]
fn subclass_is_accepted_for_its_parent() {
    assert!(mismatches(&errors(r#"takeP(use Q(1, "q"))"#)).is_empty());
}

#[test]
fn instance_and_static_methods_are_checked() {
    let found = mismatches(&errors(r#"
        use C().add("deux", 3)
        C::sadd(1, true)
    "#));
    assert_eq!(found, vec![
        ("C::add".to_string(), "a".to_string(), "int".to_string(), "string".to_string()),
        ("C::sadd".to_string(), "b".to_string(), "int".to_string(), "bool".to_string()),
    ]);
}

#[test]
fn interface_method_is_checked() {
    assert_eq!(mismatches(&errors(r#"
        var s:Shape = use Sq()
        s.area("x")
    "#)), one("Shape::area", "scale", "int", "string"));
}

#[test]
fn constructor_and_inherited_constructor_are_checked() {
    let found = mismatches(&errors(r#"
        use P("texte")
        use Q(1, 2)
    "#));
    assert_eq!(found, vec![
        ("P::init".to_string(), "x".to_string(), "int".to_string(), "string".to_string()),
        ("Q::init".to_string(), "label".to_string(), "string".to_string(), "int".to_string()),
    ]);
}

#[test]
fn parent_init_is_checked() {
    let src = r#"
        class P {
            public property x:int
            init(x:int) { self.x = x }
        }
        class R extends P {
            init() { parent::init("zéro") }
        }
        function main(): int { return 0 }
    "#;
    assert_eq!(mismatches(&errors_in(src)), one("P::init", "x", "int", "string"));
}

#[test]
fn builtin_argument_type_is_checked() {
    assert_eq!(mismatches(&errors(r#"Math::abs("texte")"#)), one("Math::abs", "n", "int", "string"));
}

#[test]
fn variadic_elements_are_checked() {
    assert_eq!(mismatches(&errors(r#"sum(1, 2, "trois")"#)), one("sum", "xs", "int", "string"));
    assert!(mismatches(&errors("sum(1, 2, 3)")).is_empty());
}

#[test]
fn null_is_accepted_for_nullable_parameter() {
    assert!(mismatches(&errors(r#"
        opt(1, null)
        opt(1, "x")
        opt(1)
    "#)).is_empty());
}

#[test]
fn default_value_type_is_checked() {
    let src = r#"
        function bad(n:int = "zéro"): int { return n }
        function main(): int { return 0 }
    "#;
    assert_eq!(mismatches(&errors_in(src)), one("bad", "n", "int", "string"));
}
