/// Codegen Cranelift — génération de code natif AOT
///
/// Pipeline :
///   IrModule  →  CraneliftEmitter  →  fichier objet `.o`  →  linkage
pub mod emit;
pub mod link;
pub mod runtime;
