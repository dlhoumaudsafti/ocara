/// Tests unitaires — `struct` : parsing (constructeur généré) et
/// `core::structs::expand_structs` (E51-E53, constructeur hérité) — voir
/// docs/roadmap.d/langage-struct-value-type.md.
use crate::core::structs::expand_structs;
use crate::parsing::ast::{ClassMember, Program};
use crate::parsing::{lexer::Lexer, parser::Parser};

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn constructor_params(program: &Program, class: &str) -> Vec<(String, bool)> {
    let decl = program.classes.iter().find(|c| c.name == class).expect("class");
    decl.members.iter()
        .find_map(|m| match m {
            ClassMember::Constructor { params, .. } => Some(params.iter().map(|p| (p.name.clone(), p.default_value.is_some())).collect()),
            _ => None,
        })
        .expect("constructor")
}

fn expand_error(src: &str) -> String {
    let mut program = parse(src);
    expand_structs(&mut program).expect_err("expected an error").1
}

#[test]
fn struct_gets_constructor_from_fields_with_defaults() {
    let program = parse("struct P { x:int\n public property y:string = \"a\"\n public const K:int = 1 }");
    let decl = &program.classes[0];
    assert!(decl.is_struct);
    assert_eq!(constructor_params(&program, "P"), vec![("x".into(), false), ("y".into(), true)]);
}

#[test]
fn struct_rejects_methods_at_parse_time() {
    let tokens = Lexer::new("struct P { x:int\n public method f():int { return 1 } }").tokenize().expect("lex ok");
    assert!(Parser::new(tokens).parse_program().is_err());
}

#[test]
fn child_struct_constructor_starts_with_parent_fields() {
    let mut program = parse("struct B { a:int\n b:int = 2 }\nstruct M extends B { c:int }\nstruct C extends M { d:int = 4 }");
    expand_structs(&mut program).expect("ok");
    assert_eq!(
        constructor_params(&program, "C"),
        vec![("a".into(), false), ("b".into(), true), ("c".into(), false), ("d".into(), true)]
    );
    let decl = program.classes.iter().find(|c| c.name == "C").unwrap();
    let Some(ClassMember::Constructor { body, .. }) = decl.members.iter().find(|m| matches!(m, ClassMember::Constructor { .. })) else { panic!() };
    assert_eq!(body.stmts.len(), 4);
}

#[test]
fn private_field_is_rejected() {
    assert!(expand_error("struct P { private x:int }").contains("'private' is not allowed"));
}

#[test]
fn struct_cannot_extend_class_nor_be_extended_by_class() {
    assert!(expand_error("class C { }\nstruct P extends C { x:int }").contains("can only extend another struct"));
    assert!(expand_error("struct P { x:int }\nclass C extends P { }").contains("can only be extended by another struct"));
}

#[test]
fn inherited_field_cannot_be_redeclared() {
    assert!(expand_error("struct P { x:int }\nstruct Q extends P { x:int }").contains("already declared by a parent struct"));
}
