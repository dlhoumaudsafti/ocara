/// Tests unitaires — champ inconnu refusé en écriture comme en lecture, sur
/// une classe, un `String`/`Array`/`Map` et une instance de générique (voir
/// docs/roadmap.d/sema-affectation-champ-inexistant.md).
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

/// `(classe, champ)` de chaque `FieldNotFound` pour `main_body`.
fn unknown_fields(main_body: &str) -> Vec<(String, String)> {
    let src = format!(r#"
        class Base {{
            public property inherited:int
        }}
        class P extends Base {{
            public property x:int
            public method size(): int {{ return 1 }}
        }}
        generic Box<T> {{
            public property v:T
            init(v:T) {{ self.v = v }}
        }}
        generic Child<T> extends Base {{
            public property v:T
            init(v:T) {{ self.v = v }}
        }}
        function main(): int {{
            var p:P = use P()
            var q:P|null = null
            var s:string = "ab"
            var a:array<int> = [1]
            var m:map<string, int> = {{"A": 1}}
            var b:Box<int> = use Box<int>(1)
            var c:Child<int> = use Child<int>(1)
            {}
            return 0
        }}
    "#, main_body);
    let program = parse(&src);
    let mut symbols = SymbolTable::new();
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors.iter().filter_map(|e| match e {
        SemaError::FieldNotFound { class, field, .. } => Some((class.clone(), field.clone())),
        _ => None,
    }).collect()
}

fn pair(class: &str, field: &str) -> (String, String) {
    (class.to_string(), field.to_string())
}

#[test]
fn assigning_an_unknown_field_is_refused() {
    assert_eq!(unknown_fields("p.nope = 5"), vec![pair("P", "nope")]);
    assert_eq!(unknown_fields("q.nope = 5"), vec![pair("P", "nope")]);
    assert_eq!(unknown_fields("s.x = 5"), vec![pair("String", "x")]);
    assert_eq!(unknown_fields("a.x = 5"), vec![pair("Array", "x")]);
    assert_eq!(unknown_fields("m.A = 5"), vec![pair("Map", "A")]);
    assert_eq!(unknown_fields("b.nope = 5"), vec![pair("Box", "nope")]);
}

#[test]
fn a_method_name_is_not_an_assignable_field() {
    assert_eq!(unknown_fields("p.size = 5"), vec![pair("P", "size")]);
    assert_eq!(unknown_fields("m.size = 5"), vec![pair("Map", "size")]);
}

#[test]
fn compound_assignment_and_increment_are_refused_too() {
    assert_eq!(unknown_fields("p.nope += 5"), vec![pair("P", "nope")]);
    assert!(unknown_fields("p.nope++").contains(&pair("P", "nope")));
}

#[test]
fn reading_an_unknown_field_of_a_generic_is_refused() {
    assert_eq!(unknown_fields("var n:int = b.nope"), vec![pair("Box", "nope")]);
}

#[test]
fn existing_and_inherited_fields_stay_accepted() {
    assert!(unknown_fields("p.x = 5\n p.inherited = 6\n q.x = 7").is_empty());
    assert!(unknown_fields("b.v = 5\n c.v = 6\n c.inherited = 7\n var n:int = c.inherited").is_empty());
    assert!(unknown_fields("var n:int = p.size()").is_empty());
}
