/// Tests unitaires — docs/roadmap.d/langage-interface-method-modifiers.md.
///
/// Deux choses vérifiées ici, séparément des tests de parsing
/// (`src/parsing/parser.d/tests.rs`, qui couvrent les 12 combinaisons
/// grammaticales) :
///
/// 1. `register_interface` propage bien `InterfaceMethod::is_async` dans
///    `FuncSig` (comme il le fait déjà pour `is_static`).
/// 2. Un bug PRÉEXISTANT, sans rapport avec la grammaire elle-même, trouvé
///    en implémentant la vérification de conformité `is_async` pour
///    `implements` : `register_class`/`register_module`/`register_generic`
///    codaient TOUS EN DUR `is_async: false` pour une méthode, quelle que
///    soit sa déclaration réelle (`register_function`, pour les fonctions
///    LIBRES, le fait déjà correctement depuis toujours) — sans conséquence
///    observable jusqu'ici car rien ne consultait `FuncSig.is_async` pour
///    une méthode (le codegen `async` de classe utilise un mécanisme
///    entièrement différent, basé sur l'AST directement, voir
///    `src/lower/builder.d/program.rs`), mais qui aurait rendu la nouvelle
///    vérification de conformité (`src/main.rs`, comparant `class_sig.is_async`
///    à `iface_sig.is_async`) TOUJOURS en échec dès qu'une interface exige
///    `async` — confirmé par reproduction avant correctif (`public async
///    method` sur la classe implémentante, pourtant conforme, était rejeté
///    comme si elle ne l'était pas).
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::symbols::SymbolTable;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

#[test]
fn register_interface_propagates_is_async_to_func_sig() {
    let program = parse("interface Fetcher { async method fetch(): int }");
    let mut symbols = SymbolTable::new();
    for decl in &program.interfaces { symbols.register_interface(decl); }
    let iface = symbols.lookup_interface("Fetcher").expect("Fetcher must be registered");
    assert_eq!(iface.methods.get("fetch").map(|s| s.is_async), Some(true));
}

#[test]
fn register_interface_non_async_method_has_is_async_false() {
    let program = parse("interface Fetcher { method fetch(): int }");
    let mut symbols = SymbolTable::new();
    for decl in &program.interfaces { symbols.register_interface(decl); }
    let iface = symbols.lookup_interface("Fetcher").expect("Fetcher must be registered");
    assert_eq!(iface.methods.get("fetch").map(|s| s.is_async), Some(false));
}

/// Le bug préexistant décrit en tête de fichier, pour une CLASSE : avant
/// correctif, ce test aurait échoué (`is_async` valait toujours `Some(false)`
/// même ici).
#[test]
fn register_class_propagates_is_async_to_func_sig() {
    let program = parse(
        "class Impl {\n\
             public async method fetch(): int { return 1 }\n\
         }\n",
    );
    let mut symbols = SymbolTable::new();
    for decl in &program.classes { symbols.register_class(decl); }
    let info = symbols.lookup_class("Impl").expect("Impl must be registered");
    assert_eq!(info.methods.get("fetch").map(|s| s.is_async), Some(true));
}

/// Même bug, pour un `generic`.
#[test]
fn register_generic_propagates_is_async_to_func_sig() {
    let program = parse(
        "generic Impl<T> {\n\
             public async method fetch(): int { return 1 }\n\
         }\n",
    );
    let mut symbols = SymbolTable::new();
    for decl in &program.generics { symbols.register_generic(decl); }
    let info = symbols.lookup_generic("Impl").expect("Impl must be registered");
    assert_eq!(info.methods.get("fetch").map(|s| s.is_async), Some(true));
}

/// Même bug, pour un `module` (mixin).
#[test]
fn register_module_propagates_is_async_to_func_sig() {
    let program = parse(
        "module Mixin {\n\
             public async method fetch(): int { return 1 }\n\
         }\n",
    );
    let mut symbols = SymbolTable::new();
    for decl in &program.modules { symbols.register_module(decl); }
    let info = symbols.modules.get("Mixin").expect("Mixin must be registered");
    assert_eq!(info.methods.get("fetch").map(|s| s.is_async), Some(true));
}

/// Non-régression : une méthode de classe NON async doit rester `is_async: false`
/// (le correctif ne doit jamais mettre `true` par erreur).
#[test]
fn register_class_non_async_method_has_is_async_false() {
    let program = parse(
        "class Impl {\n\
             public method fetch(): int { return 1 }\n\
         }\n",
    );
    let mut symbols = SymbolTable::new();
    for decl in &program.classes { symbols.register_class(decl); }
    let info = symbols.lookup_class("Impl").expect("Impl must be registered");
    assert_eq!(info.methods.get("fetch").map(|s| s.is_async), Some(false));
}
