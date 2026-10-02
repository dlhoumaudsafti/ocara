/// Tests unitaires — initialiseurs de `property` : désucrage par le parser
/// et `init()` implicite hérité (`core::property_init`) — voir
/// docs/roadmap.d/langage-property-initializer.md.
use crate::core::property_init::complete_implicit_inits;
use crate::parsing::ast::{ClassMember, Expr, Program, Stmt};
use crate::parsing::{lexer::Lexer, parser::Parser};

fn parse(src: &str) -> Result<Program, String> {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().map_err(|e| e.message)
}

fn ctor<'a>(program: &'a Program, class: &str) -> (&'a Vec<crate::parsing::ast::Param>, &'a Vec<Stmt>) {
    let decl = program.classes.iter().find(|c| c.name == class).expect("class");
    decl.members.iter().find_map(|m| match m {
        ClassMember::Constructor { params, body, .. } => Some((params, &body.stmts)),
        _ => None,
    }).expect("constructor")
}

fn assigned_field(stmt: &Stmt) -> Option<&str> {
    match stmt {
        Stmt::Assign { target: Expr::Field { field, .. }, .. } => Some(field.as_str()),
        _ => None,
    }
}

#[test]
fn initializers_are_prepended_to_init_in_declaration_order() {
    let program = parse("class A {\n public property x:int = 1\n public property y:int = 2\n init() { self.y = 5 }\n}").unwrap();
    let (_, stmts) = ctor(&program, "A");
    let fields: Vec<_> = stmts.iter().filter_map(assigned_field).collect();
    assert_eq!(fields, vec!["x", "y", "y"]);
    assert!(!program.classes[0].implicit_init);
}

#[test]
fn init_is_synthesized_when_absent() {
    let program = parse("class A {\n public property x:int = 1\n}").unwrap();
    let (params, stmts) = ctor(&program, "A");
    assert!(params.is_empty());
    assert_eq!(stmts.iter().filter_map(assigned_field).collect::<Vec<_>>(), vec!["x"]);
    assert!(program.classes[0].implicit_init);
}

#[test]
fn implicit_init_inherits_parent_constructor_and_calls_it_first() {
    let mut program = parse("class Base {\n public property id:int\n init(id:int) { self.id = id }\n}\nclass Child extends Base {\n public property extra:int = 7\n}").unwrap();
    complete_implicit_inits(&mut program);
    let (params, stmts) = ctor(&program, "Child");
    assert_eq!(params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["id"]);
    assert!(matches!(&stmts[0], Stmt::Expr(Expr::StaticCall { class, method, args, .. }) if class == "<parent>" && method == "init" && args.len() == 1));
    assert_eq!(assigned_field(&stmts[1]), Some("extra"));
}

#[test]
fn self_in_initializer_is_rejected() {
    let err = parse("class A {\n public property x:int = 1\n public property y:int = self.x\n}").unwrap_err();
    assert!(err.contains("cannot use 'self' or 'parent'"), "{}", err);
}

#[test]
fn module_property_initializer_is_rejected() {
    let err = parse("module M {\n public property x:int = 1\n}").unwrap_err();
    assert!(err.contains("cannot have an initializer"), "{}", err);
}
