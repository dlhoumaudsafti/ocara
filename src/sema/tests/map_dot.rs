/// Tests unitaires — notation pointée sur une map (`truc.A` ≡ `truc["A"]`),
/// voir src/sema/map_dot.rs et `docs/EBNF.md` §7.4.
use crate::core::named_args::rewrite_program;
use crate::parsing::ast::{Expr, Literal, Program, Stmt};
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::map_dot::is_dot_key;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn check(main_body: &str) -> (Vec<SemaError>, Program) {
    let src = format!(r#"
        class Car {{
            public property brand:string = "x"
        }}
        class Holder {{
            public property options:map<string, bool> = {{"debug": true}}
            public method debug(): bool {{ return self.options.debug }}
        }}
        generic Box<T> {{
            public property v:T
            init(v:T) {{ self.v = v }}
            public method read(): int {{ return self.v.A }}
        }}
        function main(): int {{
            var m:map<string, int> = {{"A": 1}}
            var cfg:map<string, map<string, string>> = {{"db": {{"host": "h"}}}}
            var n:map<string, int>|null = null
            var ints:map<int, string> = {{1: "a"}}
            {}
            return 0
        }}
    "#, main_body);
    let tokens = Lexer::new(&src).tokenize().expect("lex ok");
    let mut program = Parser::new(tokens).parse_program().expect("parse ok");
    let mut symbols = SymbolTable::new();
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    let errors = std::mem::take(&mut checker.errors);
    let rewrites = std::mem::take(&mut checker.rewrites);
    if errors.is_empty() {
        rewrite_program(&mut program, &rewrites).expect("rewrite ok");
    }
    (errors, program)
}

fn errors(main_body: &str) -> Vec<SemaError> {
    check(main_body).0
}

/// Valeur de la `index`-ième instruction `var` ajoutée au corps de `main`.
fn added_var(program: &Program, index: usize) -> &Expr {
    let main = program.functions.iter().find(|f| f.name == "main").expect("main");
    match &main.body.stmts[4 + index] {
        Stmt::Var { value, .. } => value,
        other => panic!("expected a var, found {:?}", other),
    }
}

fn is_index_with_key(expr: &Expr, key: &str) -> bool {
    matches!(expr, Expr::Index { index, .. } if matches!(index.as_ref(), Expr::Literal(Literal::String(k), _) if k == key))
}

#[test]
fn dot_keys_follow_the_ascii_identifier_rule() {
    for key in ["A", "_", "_a1", "host", "size"] {
        assert!(is_dot_key(key), "{}", key);
    }
    for key in ["", "1a", "a-b", "a b", "été"] {
        assert!(!is_dot_key(key), "{}", key);
    }
}

#[test]
fn read_is_rewritten_to_an_index_with_a_literal_key() {
    let (errors, program) = check("var a:int = m.A");
    assert!(errors.is_empty(), "{:?}", errors);
    assert!(is_index_with_key(added_var(&program, 0), "A"));
}

#[test]
fn chained_maps_are_rewritten_link_by_link() {
    let (errors, program) = check("var h:string = cfg.db.host\n var p:string = cfg.db[\"port\"]");
    assert!(errors.is_empty(), "{:?}", errors);
    match added_var(&program, 0) {
        Expr::Index { object, index, .. } => {
            assert!(matches!(index.as_ref(), Expr::Literal(Literal::String(k), _) if k == "host"));
            assert!(is_index_with_key(object, "db"));
        }
        other => panic!("expected an index, found {:?}", other),
    }
    assert!(is_index_with_key(added_var(&program, 1), "port"));
}

#[test]
fn write_compound_assignment_and_increment_are_accepted() {
    assert!(errors("m.A = 3\n m.B += 2\n m.C++\n n.A = 1").is_empty());
}

#[test]
fn value_type_is_checked_on_write_like_an_index() {
    let found = errors("m.A = \"x\"\n m[\"B\"] = \"y\"");
    let mismatches = found.iter().filter(|e| matches!(e, SemaError::TypeMismatch { .. })).count();
    assert_eq!(mismatches, 2, "{:?}", found);
}

#[test]
fn read_has_the_value_type() {
    let found = errors("var s:string = m.A");
    assert!(found.iter().any(|e| matches!(e, SemaError::TypeMismatch { .. })), "{:?}", found);
}

#[test]
fn literal_key_type_applies_to_the_dot_notation() {
    let body = "var car:Car = use Car()\n const d:map<string, Car|string> = {\"car\": car, \"html\": \"\"}\n var c:Car = d.car\n var h:string = d.html";
    assert!(errors(body).is_empty(), "{:?}", errors(body));
    let found = errors(&format!("{}\n d.html = car", body));
    assert!(found.iter().any(|e| matches!(e, SemaError::TypeMismatch { .. })), "{:?}", found);
}

#[test]
fn method_name_without_call_is_a_key_with_call_a_method() {
    let (errors, program) = check("var a:int = m.size\n var b:int = m.size()");
    assert!(errors.is_empty(), "{:?}", errors);
    assert!(is_index_with_key(added_var(&program, 0), "size"));
    assert!(!matches!(added_var(&program, 1), Expr::Index { .. }));
}

#[test]
fn map_field_of_an_object_is_rewritten() {
    let (errors, program) = check("");
    assert!(errors.is_empty(), "{:?}", errors);
    let holder = program.classes.iter().find(|c| c.name == "Holder").expect("Holder");
    let body = holder.members.iter().find_map(|m| match m {
        crate::parsing::ast::ClassMember::Method { decl, .. } if decl.name == "debug" => Some(&decl.body),
        _ => None,
    }).expect("debug");
    assert!(matches!(&body.stmts[0], Stmt::Return { value: Some(v), .. } if is_index_with_key(v, "debug")));
}

#[test]
fn non_string_keys_are_refused() {
    let found = errors("var s:string = ints.A");
    assert!(found.iter().any(|e| matches!(e, SemaError::MapDotKeyType { .. })), "{:?}", found);
}

#[test]
fn non_ascii_key_is_refused_with_the_index_form() {
    let found = errors("var a:int = m.été");
    let message = found.iter().find(|e| matches!(e, SemaError::MapDotInvalidKey { .. })).map(|e| e.message());
    assert!(message.is_some_and(|m| m.contains("[\"été\"]")), "{:?}", found);
}

#[test]
fn map_dot_through_a_type_parameter_is_refused() {
    let found = errors("var b:Box<map<string, int>> = use Box<map<string, int>>(m)");
    assert!(found.iter().any(|e| matches!(e, SemaError::InGenericInstance { message, .. } if message.contains("type parameter"))), "{:?}", found);
}
