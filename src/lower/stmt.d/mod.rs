/// Module stmt : Lowering des statements

pub mod block;
pub mod statements;
pub mod ownership;
pub mod element_escape;
pub mod object_owners;
mod object_ast;
pub mod object_facts;

#[cfg(test)]
mod tests;

// Re-exports publics
pub use block::lower_block;
