/// Analyse sémantique Ocara v1.0
///
/// Pipeline :
///   1. `SymbolTable`  — collecte tous les symboles (imports, types, fonctions)
///   2. `TypeChecker`  — vérifie les types de chaque expression / statement
///   3. `SemaError`    — erreurs sémantiques rapportées
pub mod error;
pub mod scope;
pub mod symbols;
pub mod typecheck;
