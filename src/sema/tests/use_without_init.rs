/// Tests unitaires — `use C(args)` sur une classe utilisateur sans `init`
/// (E60) — voir docs/roadmap.d/sema-use-args-without-init.md.
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
    let src = format!(r#"
        import ocara.Exception
        class C {{
            public property name:string
        }}
        class D extends C {{
        }}
        class E extends C {{
            init(n:string) {{
                self.name = n
            }}
        }}
        class F extends E {{
        }}
        class MyErr extends Exception {{
        }}
        function main(): int {{
            {}
            return 0
        }}
    "#, main_body);
    let program = parse(&src);
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

fn args_without_constructor(errs: &[SemaError]) -> Vec<String> {
    errs.iter().filter_map(|e| match e {
        SemaError::ArgsWithoutConstructor { class, .. } => Some(class.clone()),
        _ => None,
    }).collect()
}

#[test]
fn arguments_without_init_are_rejected() {
    let errs = errors(r#"var c:C = use C("x")
            var d:D = use D("y")
            var e:MyErr = use MyErr("boom", 2)"#);
    assert_eq!(args_without_constructor(&errs), vec!["C", "D", "MyErr"], "{:?}", errs);
}

#[test]
fn no_arguments_and_inherited_init_are_accepted() {
    let errs = errors(r#"var c:C = use C()
            var f:F = use F("ok")
            var x:Exception = use Exception("boom", 3)"#);
    assert!(errs.is_empty(), "{:?}", errs);
}
