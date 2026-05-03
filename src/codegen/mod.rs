/// Codegen Cranelift — génération de code natif AOT
///
/// Pipeline :
///   IrModule  →  CraneliftEmitter  →  fichier objet `.o`  →  linkage
#[path = "desc.d/mod.rs"]
pub mod desc;
#[path = "emit.d/mod.rs"]
pub mod emit;
pub mod link;
pub mod runtime;
