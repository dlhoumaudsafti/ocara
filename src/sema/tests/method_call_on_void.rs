/// Tests unitaires — E36, appel de méthode chaîné sur le résultat `void`
/// d'un appel précédent (`self.port(8080).workers(4)`) — voir
/// docs/roadmap.d/langage-appel-methode-sur-void-accepte.md.
///
/// Avant ce diagnostic, `type_class_name(Type::Void)` retombait sur `None`
/// comme n'importe quel type sans classe associée, silencieusement permissif
/// (retourne `Type::Mixed` sans vérifier le reste de la chaîne) — reproduit
/// dans `examples/advanced/tauri_httpserver/configs/Server.oc`
/// (`.workers(4)`/`.rootPath(...)` jamais exécutés, sans la moindre erreur
/// de compilation).
///
/// Repose sur un vrai pipeline lex → parse → symboles → typecheck (comme
/// `builtin_alias_resolution.rs`/`self_const_resolution.rs`).
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

fn method_call_on_void(errors: &[SemaError]) -> Option<&str> {
    errors.iter().find_map(|e| match e {
        SemaError::MethodCallOnVoid { method, .. } => Some(method.as_str()),
        _ => None,
    })
}

const SERVER_CLASS: &str = r#"
    class Server {
        public method port(p:int): void {
        }
        public method workers(n:int): void {
        }
    }
"#;

/// Le cas exact du bug : `self.port(8080).workers(4)` — `port` retourne
/// `void`, chaîner `.workers(4)` dessus doit être rejeté.
#[test]
fn chained_call_on_void_return_is_rejected() {
    let src = format!(
        "{SERVER_CLASS}\nfunction main(): int {{ var s:Server = use Server() s.port(8080).workers(4) return 0 }}"
    );
    let errors = check_errors(&src);
    assert_eq!(method_call_on_void(&errors), Some("workers"), "errors = {:?}", errors);
}

/// Non-régression : les DEUX mêmes appels, mais séparés (pas chaînés) —
/// aucune erreur, c'est exactement le contournement recommandé.
#[test]
fn separate_calls_on_void_returning_methods_are_accepted() {
    let src = format!(
        "{SERVER_CLASS}\nfunction main(): int {{ var s:Server = use Server() s.port(8080) s.workers(4) return 0 }}"
    );
    let errors = check_errors(&src);
    assert!(method_call_on_void(&errors).is_none(), "errors = {:?}", errors);
}

/// Non-régression : chaîner sur le retour d'une méthode qui retourne
/// RÉELLEMENT quelque chose (pas `void`) doit continuer à fonctionner
/// normalement — le rejet est spécifique à `void`, pas à tout chaînage.
#[test]
fn chained_call_on_non_void_return_is_still_accepted() {
    let src = r#"
        class Builder {
            public method withName(n:string): Builder {
                return self
            }
            public method build(): string {
                return "built"
            }
        }
        function main(): int {
            var b:Builder = use Builder()
            scoped r:string = b.withName("x").build()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(method_call_on_void(&errors).is_none(), "errors = {:?}", errors);
}
