/// Module du parser

pub mod types;
mod primitives;
mod program;
mod imports;
mod types_parsing;
mod declarations;
pub(crate) mod struct_decl;
mod property_init;
mod statements;
mod compound_assign;
mod expressions;
mod runtime;
mod recovery;
mod when;
mod architecture;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_when;
#[cfg(test)]
mod tests_depth;

pub use types::Parser;
