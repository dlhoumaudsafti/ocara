/// Tests unitaires — E38 (`SemaError::InterfaceNoWiring`) et la vérification
/// de staticité (`is_static`) ajoutée aux boucles 4d/4d-bis de `src/main.rs` —
/// voir docs/roadmap.d/langage-interface-wiring.md.
///
/// La substitution "nom nu → premier `wiring`" elle-même
/// (`core::interface_wiring::resolve_bare_interface_names`) est déjà testée
/// directement dans `src/core/interface_wiring.rs` (son propre module
/// `#[cfg(test)] mod tests`, sans dépendre du typecheck). Ici, on vérifie le
/// comportement du TYPECHECK une fois cette passe appliquée — reproduisant
/// le même ordre de pipeline que `src/main.rs` (§4b-bis AVANT §4c/le
/// typecheck) — pour confirmer que E38 ne se déclenche QUE quand aucun
/// `wiring` n'existe, jamais quand la substitution a déjà fait son travail.
use std::collections::HashMap;
use crate::core::interface_wiring::resolve_bare_interface_names;
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

/// Même pipeline que `src/main.rs` : résolution des noms nus d'interface
/// AVANT la construction de la table des symboles/le typecheck.
fn check_errors(src: &str) -> Vec<SemaError> {
    let mut program = parse(src);

    let all_interfaces: HashMap<_, _> = program.interfaces.iter()
        .map(|i| (i.name.clone(), i.clone())).collect();
    resolve_bare_interface_names(&mut program, &all_interfaces);

    let mut symbols = SymbolTable::new();
    for decl in &program.imports    { symbols.register_import(decl); }
    for decl in &program.consts     { symbols.register_const(decl); }
    for decl in &program.interfaces { symbols.register_interface(decl); }
    for decl in &program.modules    { symbols.register_module(decl); }
    for decl in &program.enums      { symbols.register_enum(decl); }
    for decl in &program.classes    { symbols.register_class(decl); }
    for decl in &program.generics   { symbols.register_generic(decl); }
    for decl in &program.functions  { symbols.register_function(decl); }

    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

fn interface_no_wiring(errors: &[SemaError]) -> Option<&str> {
    errors.iter().find_map(|e| match e {
        SemaError::InterfaceNoWiring { name, .. } => Some(name.as_str()),
        _ => None,
    })
}

/// `use Repo()` (construction) sur une interface sans AUCUN `wiring` — E38.
#[test]
fn construction_on_interface_without_wiring_is_rejected() {
    let src = r#"
        interface Repo {
            method save(): void
        }
        function main(): int {
            var r:Repo = use Repo()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert_eq!(interface_no_wiring(&errors), Some("Repo"), "errors = {:?}", errors);
}

/// `Repo::create()` (appel statique) sur une interface sans AUCUN `wiring` — E38.
#[test]
fn static_call_on_interface_without_wiring_is_rejected() {
    let src = r#"
        interface Repo {
            static method create(): Repo
        }
        function main(): int {
            Repo::create()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert_eq!(interface_no_wiring(&errors), Some("Repo"), "errors = {:?}", errors);
}

/// Non-régression : une interface avec au moins un `wiring` ne déclenche
/// JAMAIS E38 — le nom nu a déjà été réécrit vers la classe concrète avant
/// que le typecheck ne s'exécute, donc aucune interface n'est plus "vue" ici.
#[test]
fn construction_on_interface_with_wiring_is_accepted() {
    let src = r#"
        interface Repo {
            method save(): void
            wiring PostgresRepo
        }
        class PostgresRepo implements Repo {
            public method save(): void {
            }
        }
        function main(): int {
            var r:Repo = use Repo()
            r.save()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert_eq!(interface_no_wiring(&errors), None, "errors = {:?}", errors);
}

/// Non-régression : le type annoté (`var r:Repo`) reste l'interface
/// abstraite — ce n'est ni une classe ni un générique, mais ce n'est PAS un
/// nom de classe/générique introuvable non plus (aucune erreur ne doit
/// provenir de la simple présence d'un type d'interface en annotation).
#[test]
fn interface_type_annotation_alone_is_never_flagged() {
    let src = r#"
        interface Repo {
            method save(): void
            wiring PostgresRepo
        }
        class PostgresRepo implements Repo {
            public method save(): void {
            }
        }
        function takesRepo(r:Repo): void {
        }
        function main(): int {
            var r:Repo = use Repo()
            takesRepo(r)
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(errors.is_empty(), "errors = {:?}", errors);
}

/// Staticité incompatible entre une interface et une classe qui
/// l'`implements` : l'interface exige une méthode STATIQUE, la classe la
/// déclare en INSTANCE — doit être rejeté au moment du chargement (§4d de
/// `src/main.rs`, hors du typecheck lui-même, donc pas exercé directement
/// ici) — voir plutôt `tests/interface_wiring_conformance.oc` (regression
/// end-to-end) pour ce cas précis : ce test-ci vérifie seulement que la
/// table des symboles porte bien `is_static` pour une méthode d'interface,
/// prérequis silencieux de cette vérification.
#[test]
fn interface_method_is_static_is_registered_in_symbol_table() {
    let src = r#"
        interface Repo {
            static method create(): Repo
            wiring PostgresRepo
        }
        class PostgresRepo implements Repo {
            public static method create(): Repo {
                return use PostgresRepo()
            }
        }
    "#;
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.interfaces { symbols.register_interface(decl); }
    let iface = symbols.lookup_interface("Repo").expect("Repo must be registered");
    assert_eq!(iface.methods.get("create").map(|s| s.is_static), Some(true));
}
