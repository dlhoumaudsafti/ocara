/// Module expr : Lowering des expressions

pub mod helpers;
pub mod captures;
pub mod nameless;
pub mod typeinfer;
pub mod literals;
pub mod tauri_handler;
pub mod lower;

#[cfg(test)]
mod tests;

// Re-exports publics
pub use lower::{lower_expr, lower_receiver};
pub use lower::{lower_array_literal, lower_map_key, lower_map_literal, lower_typed_call_arg};
pub use lower::hoist_closure_promotions_before_loop;
pub use typeinfer::expr_ir_type_pub;
