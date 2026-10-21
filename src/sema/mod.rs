/// Analyse sémantique Ocara v1.0
///
/// Pipeline :
///   1. `SymbolTable`  — collecte tous les symboles (imports, types, fonctions)
///   2. `TypeChecker`  — vérifie les types de chaque expression / statement
///   3. `SemaError`    — erreurs sémantiques rapportées
pub mod error;
pub mod arg_types;
pub mod blocking_assert;
pub mod convert_sugar;
pub mod duplicates;
pub mod escape;
pub mod field_visibility;
pub mod generic_check;
pub mod generic_static;
pub mod index;
pub mod map_keys;
pub mod message_emit;
pub mod mixed_string;
pub mod named_args;
pub mod narrowing;
pub mod resource_raise;
pub mod returns;
pub mod scope;
pub mod sql_injection;
pub mod symbols;
pub mod typecheck;

#[cfg(test)]
mod tests;
