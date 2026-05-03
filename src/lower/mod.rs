/// Lowering : AST → Ocara HIR
///
/// Ce module transforme le `Program` AST en `IrModule` HIR.
/// Il n'effectue pas d'analyse sémantique (déjà faite par `sema`).
#[path = "builder.d/mod.rs"]
pub mod builder;
#[path = "expr.d/mod.rs"]
pub mod expr;
#[path = "stmt.d/mod.rs"]
pub mod stmt;
