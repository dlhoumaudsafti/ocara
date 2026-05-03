/// Module expr : Lowering des expressions

pub mod helpers;
pub mod captures;
pub mod nameless;
pub mod typeinfer;
pub mod literals;
pub mod lower;

// Re-exports publics
pub use lower::lower_expr;
pub use typeinfer::expr_ir_type_pub;
