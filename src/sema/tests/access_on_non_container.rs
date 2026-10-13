/// Tests unitaires — champ lu ou indexation sur `int`/`float`/`bool`/`void`
/// (E63) et `message<T>` en paramètre de closure (E31) — voir
/// docs/roadmap.d/sema-acces-champ-index-non-classe.md et
/// docs/roadmap.d/langage-emit-iterable.md.
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
        class F {{
            public property items:array<int>
            public method doIt(): void {{}}
            public method count(): int {{ return 3 }}
        }}
        function main(): int {{
            var x:int = 42
            var b:bool = true
            var fl:float = 1.5
            var f:F = use F()
            {}
            return 0
        }}
    "#, main_body);
    let program = parse(&src);
    let mut symbols = SymbolTable::new();
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

/// `(type, champ)` de chaque E63 (`None` : indexation).
fn accesses(errs: &[SemaError]) -> Vec<(String, Option<String>)> {
    errs.iter().filter_map(|e| match e {
        SemaError::AccessOnNonContainer { type_name, field, .. } => Some((type_name.clone(), field.clone())),
        _ => None,
    }).collect()
}

#[test]
fn field_on_primitive_or_void_is_rejected() {
    let found = accesses(&errors(r#"
        var a:mixed = x.nope
        var c:mixed = b.foo
        var d:mixed = fl.foo
        var e:mixed = f.count().foo
        var g:mixed = f.doIt().x
    "#));
    assert_eq!(found, vec![
        ("int".to_string(), Some("nope".to_string())),
        ("bool".to_string(), Some("foo".to_string())),
        ("float".to_string(), Some("foo".to_string())),
        ("int".to_string(), Some("foo".to_string())),
        ("void".to_string(), Some("x".to_string())),
    ]);
}

#[test]
fn index_on_primitive_or_void_is_rejected() {
    let found = accesses(&errors("var a:mixed = x[3]\n var c:mixed = f.doIt()[0]"));
    assert_eq!(found, vec![("int".to_string(), None), ("void".to_string(), None)]);
}

#[test]
fn containers_strings_and_mixed_are_accepted() {
    let errs = errors(r#"
        var a:int = f.items[0]
        var s:string = "abc"
        var m:mixed = 3
        var n:mixed = m.field
        var o:mixed = m[1]
    "#);
    assert!(accesses(&errs).is_empty(), "{:?}", errs);
}

#[test]
fn message_closure_parameter_is_rejected() {
    let errs = errors("var g:mixed = nameless(m:message<int>): void {}");
    assert!(errs.iter().any(|e| matches!(e, SemaError::MessageAsParamType { name, .. } if name == "m")), "{:?}", errs);
}
