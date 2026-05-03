/// Module du parser

pub mod types;
mod primitives;
mod program;
mod imports;
mod types_parsing;
mod declarations;
mod statements;
mod expressions;
mod runtime;

#[cfg(test)]
mod tests;

pub use types::Parser;
