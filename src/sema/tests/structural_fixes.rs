/// Tests unitaires — correctifs structurels du 2026-10-13 : réaffectation
/// typée, `mixed` → `string`, `UnitTest::assertEnum`, appel statique de
/// `generic` (inférence, syntaxe explicite) et revérification d'une
/// instanciation de `generic`.
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn check(src: &str) -> (Vec<String>, usize, usize) {
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.enums { symbols.register_enum(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    let messages = checker.errors.iter().map(|e| e.message()).collect();
    (messages, checker.rewrites.mixed_to_string.len(), checker.rewrites.generic_static.len())
}

const BOX: &str = r#"
    generic Box<T> {
        private property v:T
        init(v:T) { self.v = v }
        public static method of(v:T): Box<T> { return use Box<T>(v) }
        public static method empty(): Box<T> { return use Box<T>(null) }
    }
"#;

const ACC: &str = r#"
    generic Acc<T> {
        private property v:T
        init(v:T) { self.v = v }
        public method minus(e:T): T { return self.v - e }
    }
"#;

#[test]
fn reassignment_is_type_checked() {
    let (errs, _, _) = check("function main(): int {\n var x:int = 1\n x = \"abc\"\n return x\n}\n");
    assert!(errs.iter().any(|e| e.contains("expected type 'int', found 'string'")), "{:?}", errs);
}

#[test]
fn mixed_used_as_string_is_converted() {
    let src = "function show(s:string): int { return 0 }\nfunction main(): int {\n var m:mixed = 42\n var s:string = m\n s = m\n show(m)\n return 0\n}\n";
    let (errs, conversions, _) = check(src);
    assert!(errs.is_empty(), "{:?}", errs);
    assert_eq!(conversions, 3);
}

#[test]
fn assert_enum_requires_an_enum_value() {
    let ok = "import ocara.UnitTest\nenum Color { RED, GREEN }\nfunction main(): int {\n UnitTest::assertEnum(Color::RED)\n return 0\n}\n";
    assert!(check(ok).0.is_empty(), "{:?}", check(ok).0);
    let ko = "import ocara.UnitTest\nfunction main(): int {\n UnitTest::assertEnum(3)\n return 0\n}\n";
    assert!(check(ko).0.iter().any(|e| e.contains("expected type 'enum'")));
}

#[test]
fn generic_static_call_infers_or_takes_explicit_type_args() {
    let src = format!("{}\nfunction main(): int {{\n var a:Box<int> = Box::of(4)\n var b:Box<string> = Box<string>::of(\"x\")\n var c:Box<int> = Box::empty()\n return 0\n}}\n", BOX);
    let (errs, _, statics) = check(&src);
    assert!(errs.is_empty(), "{:?}", errs);
    assert_eq!(statics, 3);
}

#[test]
fn generic_static_call_without_inferable_type_args_is_rejected() {
    let src = format!("{}\nfunction main(): int {{\n var e:mixed = Box::empty()\n return 0\n}}\n", BOX);
    assert!(check(&src).0.iter().any(|e| e.contains("cannot infer the type arguments of 'Box::empty'")));
}

#[test]
fn generic_instance_with_invalid_operation_is_rejected() {
    let src = format!("{}\nfunction main(): int {{\n var a:Acc<int> = use Acc<int>(1)\n var s:Acc<string> = use Acc<string>(\"x\")\n return 0\n}}\n", ACC);
    let errs = check(&src).0;
    assert!(errs.iter().any(|e| e.contains("in 'Acc<string>'") && e.contains("operator '-'")), "{:?}", errs);
    assert!(!errs.iter().any(|e| e.contains("Acc<int>")), "{:?}", errs);
}
