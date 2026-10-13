/// Tests unitaires — E54 étendu aux méthodes et constantes de classe — voir
/// docs/roadmap.d/langage-field-visibility-unchecked.md.
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

/// `(genre, nom, protected)` de chaque E54 levée sur une méthode/constante.
fn member_errors(main_body: &str) -> Vec<(String, String, bool)> {
    let src = format!(r#"
        class C {{
            private method secret(): int {{ return 2 }}
            protected static method ps(): int {{ return 7 }}
            private const SECRET:string = "abc"
            public method callSecret(): int {{ return self.secret() }}
        }}
        class D extends C {{
            public method okProtected(): int {{ return C::ps() }}
            public method koPrivate(): int {{ return self.secret() }}
        }}
        function main(): int {{
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
    let mut found: Vec<(String, String, bool)> = checker.errors.iter()
        .filter_map(|e| match e {
            SemaError::MemberNotAccessible { kind, name, protected, .. } => Some((kind.clone(), name.clone(), *protected)),
            _ => None,
        })
        .collect();
    found.sort();
    found
}

fn err(kind: &str, name: &str, protected: bool) -> (String, String, bool) {
    (kind.into(), name.into(), protected)
}

#[test]
fn private_method_from_subclass_is_rejected_but_protected_is_allowed() {
    assert_eq!(member_errors(""), vec![err("method", "secret", false)]);
}

#[test]
fn external_calls_and_constant_reads_are_rejected() {
    let found = member_errors(r#"
        var c:C = use C()
        c.secret()
        C::ps()
        var s:string = C::SECRET
    "#);
    assert_eq!(found, vec![
        err("constant", "SECRET", false),
        err("method", "ps", true),
        err("method", "secret", false),
        err("method", "secret", false),
    ]);
}

#[test]
fn public_method_reaching_private_member_is_allowed() {
    assert_eq!(member_errors("var c:C = use C()\n c.callSecret()"), vec![err("method", "secret", false)]);
}
