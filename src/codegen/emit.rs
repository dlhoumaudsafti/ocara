/// Module emit : compilation Cranelift IR → code machine

#[path = "emit.d/mod.rs"]
mod emit_impl;

pub use emit_impl::{CodegenError, CgResult, CraneliftEmitter};

