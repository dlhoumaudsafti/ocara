/// Tests unitaires — `wiring` d'une classe de test (`core::test_wiring`) :
/// parsing, placement réservé aux tests, substitution de la cible.
use std::collections::HashMap;
use std::path::Path;
use crate::core::test_wiring::{apply, check_entry, check_imported};
use crate::parsing::ast::{InterfaceDecl, Program};
use crate::parsing::{lexer::Lexer, parser::Parser};

const CONTRACT: &str = "interface Repo {\n    wiring infra.RealRepo\n    public static method name(): string\n}\n\
    class RealRepo implements Repo {\n    public static method name(): string { return \"real\" }\n}\n\
    class FakeRepo implements Repo {\n    public static method name(): string { return \"fake\" }\n}\n";

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn parse_error(src: &str) -> String {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect_err("expected a parse error").message
}

fn interfaces(program: &Program) -> HashMap<String, InterfaceDecl> {
    program.interfaces.iter().map(|i| (i.name.clone(), i.clone())).collect()
}

fn first_wiring(program: &Program, iface: &str) -> String {
    let decl = program.interfaces.iter().find(|i| i.name == iface).expect("interface");
    decl.wirings[0].simple_name().to_string()
}

#[test]
fn class_wiring_is_parsed_and_kept_out_of_members() {
    let program = parse("class T extends UnitTest {\n    wiring support.FakeRepo\n    public method aTest(): int { return 0 }\n}\n");
    let class = &program.classes[0];
    assert_eq!(class.wirings.len(), 1);
    assert_eq!(class.wirings[0].path, vec!["support".to_string(), "FakeRepo".to_string()]);
    assert_eq!(class.members.len(), 1);
}

#[test]
fn wiring_in_module_or_generic_is_rejected() {
    assert!(parse_error("module M {\n    wiring a.B\n}\n").contains("only allowed in an interface or in a unit test class"));
    assert!(parse_error("generic G<T> {\n    wiring a.B\n}\n").contains("only allowed in an interface or in a unit test class"));
}

#[test]
fn test_class_in_test_file_is_accepted() {
    let program = parse("class T extends UnitTest {\n    wiring FakeRepo\n}\n");
    assert!(check_entry(&program, Path::new("tests/RepoTest.oc")).is_ok());
    assert!(check_entry(&program, Path::new("__ocaraunit__RepoTest.oc")).is_ok());
}

#[test]
fn wiring_outside_a_unit_test_class_is_rejected() {
    let program = parse("class T {\n    wiring FakeRepo\n}\n");
    let err = check_entry(&program, Path::new("RepoTest.oc")).expect_err("not a test class");
    assert!(err.message.contains("extends UnitTest"));
}

#[test]
fn wiring_outside_a_test_file_is_rejected() {
    let program = parse("class T extends UnitTest {\n    wiring FakeRepo\n}\n");
    let err = check_entry(&program, Path::new("main.oc")).expect_err("not a test file");
    assert!(err.message.contains("*Test.oc"));
}

#[test]
fn wiring_in_an_imported_file_is_rejected() {
    let program = parse("class T extends UnitTest {\n    wiring FakeRepo\n}\n");
    assert!(check_imported(&program, Path::new("tests/support/Base.oc")).is_err());
    assert!(check_imported(&parse(CONTRACT), Path::new("infra/Repo.oc")).is_ok());
}

#[test]
fn target_becomes_first_wiring_of_its_interfaces() {
    let mut program = parse(&format!("{}class T extends UnitTest {{\n    wiring support.FakeRepo\n}}\n", CONTRACT));
    let mut all = interfaces(&program);
    apply(&mut program, &mut all, Path::new("RepoTest.oc")).expect("applied");
    assert_eq!(first_wiring(&program, "Repo"), "FakeRepo");
    assert_eq!(all["Repo"].wirings[0].simple_name(), "FakeRepo");
    assert_eq!(all["Repo"].wirings[1].simple_name(), "RealRepo");
}

#[test]
fn target_without_interface_is_rejected() {
    let src = format!("{}class Lone {{\n}}\nclass T extends UnitTest {{\n    wiring Lone\n}}\n", CONTRACT);
    let mut program = parse(&src);
    let mut all = interfaces(&program);
    let err = apply(&mut program, &mut all, Path::new("RepoTest.oc")).expect_err("no interface");
    assert!(err.message.contains("implements no interface"));
}

#[test]
fn unknown_target_is_rejected() {
    let mut program = parse("class T extends UnitTest {\n    wiring Missing\n}\n");
    let mut all = interfaces(&program);
    let err = apply(&mut program, &mut all, Path::new("RepoTest.oc")).expect_err("unknown");
    assert!(err.message.contains("target class not found"));
}

#[test]
fn interface_replaced_twice_is_rejected() {
    let src = format!(
        "{}class OtherFake implements Repo {{\n    public static method name(): string {{ return \"other\" }}\n}}\n\
         class T extends UnitTest {{\n    wiring FakeRepo\n    wiring OtherFake\n}}\n", CONTRACT);
    let mut program = parse(&src);
    let mut all = interfaces(&program);
    let err = apply(&mut program, &mut all, Path::new("RepoTest.oc")).expect_err("twice");
    assert!(err.message.contains("replaced twice"));
}
