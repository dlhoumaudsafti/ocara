pub mod alias_resolve;
#[path = "analysis.d/mod.rs"]
pub mod analysis;
pub mod cli;
pub mod diagnostics;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_structs;
#[cfg(test)]
mod tests_property_init;
#[cfg(test)]
mod tests_render_file;
pub mod interface_wiring;
pub mod monomorph;
pub mod named_args;
pub mod structs;
pub mod property_init;
pub mod render_file;
pub mod runtime_expand;
pub mod source;
