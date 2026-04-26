/// Lowering : AST → Ocara HIR
///
/// Ce module transforme le `Program` AST en `IrModule` HIR.
/// Il n'effectue pas d'analyse sémantique (déjà faite par `sema`).
pub mod builder;
pub mod expr;
pub mod stmt;
