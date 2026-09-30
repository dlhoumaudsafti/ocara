/// Tests unitaires — E54, visibilité des champs `private`/`protected` à la
/// lecture comme à l'affectation — voir
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

/// `(champ, protected)` de chaque E54 levée.
fn visibility_errors(src: &str) -> Vec<(String, bool)> {
    let mut program = parse(src);
    crate::core::structs::expand_structs(&mut program).expect("structs ok");
    let mut symbols = SymbolTable::new();
    for decl in &program.modules { symbols.register_module(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors.iter()
        .filter_map(|e| match e {
            SemaError::FieldNotAccessible { field, protected, .. } => Some((field.clone(), *protected)),
            _ => None,
        })
        .collect()
}

const BASE: &str = r#"
    class Base {
        protected property p:int
        private property q:int
        init() {
            self.p = 1
            self.q = 2
        }
        public method sum(other:Base): int { return other.p + other.q }
    }
"#;

#[test]
fn external_read_of_private_and_protected_is_rejected() {
    let src = format!("{}\nfunction main(): int {{ var b:Base = use Base()\n return b.p + b.q }}", BASE);
    assert_eq!(visibility_errors(&src), vec![("p".into(), true), ("q".into(), false)]);
}

#[test]
fn external_assignment_and_increment_are_rejected() {
    let src = format!("{}\nfunction main(): int {{ var b:Base = use Base()\n b.p = 3\n b.q++\n return 0 }}", BASE);
    assert_eq!(visibility_errors(&src), vec![("p".into(), true), ("q".into(), false)]);
}

#[test]
fn declaring_class_accesses_other_instance_fields() {
    assert!(visibility_errors(&format!("{}\nfunction main(): int {{ return 0 }}", BASE)).is_empty());
}

#[test]
fn subclass_reaches_protected_but_not_private() {
    let src = format!(r#"{}
        class Child extends Base {{
            public method readP(): int {{ return self.p }}
            public method readQ(): int {{ return self.q }}
        }}
        function main(): int {{ return 0 }}"#, BASE);
    assert_eq!(visibility_errors(&src), vec![("q".into(), false)]);
}

#[test]
fn module_private_field_belongs_to_the_using_class() {
    let src = r#"
        module Tagged {
            private property tag:string
            public method setTag(t:string): void { self.tag = t }
        }
        class User modules Tagged {
            public method rename(): void { self.tag = "x" }
        }
        function main(): int { return 0 }
    "#;
    assert!(visibility_errors(src).is_empty());
}

#[test]
fn struct_protected_field_is_visible_to_derived_struct_constructor_only() {
    let src = r#"
        struct SBase { x:int
            protected hidden:int = 5 }
        struct SChild extends SBase { y:int = 0 }
        function main(): int {
            var s:SChild = use SChild(x: 1, hidden: 7)
            return s.hidden
        }
    "#;
    assert_eq!(visibility_errors(src), vec![("hidden".into(), true)]);
}
