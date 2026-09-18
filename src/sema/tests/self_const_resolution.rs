/// Tests unitaires — `self::CONST_NAME` (lecture d'une constante de classe
/// via `self::`) échouait TOUJOURS, y compris dans le constructeur, avec
/// "undefined symbol '<self>::CONST_NAME'".
///
/// `Expr::StaticConst` (le nœud AST de `self::X`/`parent::X`) cherchait la
/// constante avec `class` encore littéralement égal à `"<self>"` — la
/// résolution vers la classe courante n'intervenait que plus bas, dans le
/// chemin de repli réservé aux méthodes statiques référencées sans appel
/// (`ClassName::myStatic`), jamais pour la recherche de constante elle-même.
/// `Expr::StaticCall` (l'appel de méthode, `self::method()`) résolvait déjà
/// `<self>` correctement dès l'entrée — seul le nœud `StaticConst` avait ce
/// bug.
///
/// Repose sur un vrai pipeline lex → parse → symboles → typecheck (comme
/// `builtin_alias_resolution.rs`) : le bug dépend de la classe courante
/// (`current_class`) telle que suivie pendant le typecheck réel d'un corps de
/// méthode/constructeur, pas simulable via un graphe d'AST synthétique.
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn check_errors(src: &str) -> Vec<SemaError> {
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.consts { symbols.register_const(decl); }
    for decl in &program.interfaces { symbols.register_interface(decl); }
    for decl in &program.modules { symbols.register_module(decl); }
    for decl in &program.enums { symbols.register_enum(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
    for decl in &program.functions { symbols.register_function(decl); }

    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

fn has_undefined_symbol(errors: &[SemaError]) -> bool {
    errors.iter().any(|e| matches!(e, SemaError::UndefinedSymbol { .. }))
}

/// `self::CONST_NAME` depuis une méthode d'instance normale (pas le
/// constructeur).
#[test]
fn self_const_resolves_from_instance_method() {
    let src = r#"
        class Foo {
            private const PATH:string = "./x"

            public method show(): string {
                return self::PATH
            }
        }

        function main(): int {
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(!has_undefined_symbol(&errors), "errors = {:?}", errors);
}

/// Même bug depuis le constructeur (`init()`) — le cas d'usage le plus
/// courant en pratique (`self.champ = self::CONST`, ex.
/// `SQLite::open(self::DB_PATH)`).
#[test]
fn self_const_resolves_from_constructor() {
    let src = r#"
        class Foo {
            private const PATH:string = "./x"
            private property path:string

            init() {
                self.path = self::PATH
            }
        }

        function main(): int {
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(!has_undefined_symbol(&errors), "errors = {:?}", errors);
}

/// Régression : une VRAIE constante inconnue référencée via `self::` doit
/// rester rejetée — résoudre `<self>` ne doit pas transformer ce diagnostic
/// en no-op général. Le message doit aussi refléter la classe résolue
/// (`Foo::NOPE`), pas le nom littéral non résolu (`<self>::NOPE`).
#[test]
fn self_unknown_const_is_still_rejected_with_resolved_class_name() {
    let src = r#"
        class Foo {
            private const PATH:string = "./x"

            public method show(): string {
                return self::NOPE
            }
        }

        function main(): int {
            return 0
        }
    "#;
    let errors = check_errors(src);
    let msg = errors.iter().find_map(|e| match e {
        SemaError::UndefinedSymbol { name, .. } => Some(name.clone()),
        _ => None,
    });
    assert_eq!(msg.as_deref(), Some("Foo::NOPE"), "errors = {:?}", errors);
}
