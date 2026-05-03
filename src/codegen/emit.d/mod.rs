/// Module emit : compilation Cranelift

pub mod error;
pub mod helpers;
pub mod emitter;
#[path = "instructions.d/mod.rs"]
pub mod instructions;

// Re-exports publics
#[allow(unused_imports)]
pub use error::{CodegenError, CgResult};
pub use emitter::CraneliftEmitter;
