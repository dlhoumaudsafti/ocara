/// Ocara HIR — High-level Intermediate Representation
///
/// Représentation intermédiaire typée, plate (pas d'AST récursif),
/// proche de la forme SSA mais restant lisible.
/// Chaque fonction est représentée comme une liste de BasicBlock.
pub mod types;
pub mod inst;
pub mod func;
pub mod module;
