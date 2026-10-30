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
#[cfg(test)]
mod tests_homonyms;
#[cfg(test)]
mod tests_architecture;
#[cfg(test)]
mod tests_test_wiring;
#[cfg(test)]
mod tests_leak_sites;
#[cfg(test)]
mod tests_cli;
#[cfg(test)]
mod tests_monomorph;
pub mod interface_wiring;
pub mod leak_sites;
pub mod monomorph;
pub mod named_args;
pub mod structs;
pub mod property_init;
pub mod render_file;
pub mod runtime_expand;
pub mod source;
pub mod test_wiring;
pub mod ui_file;
#[path = "ui_schema.d/mod.rs"]
pub mod ui_schema;
#[cfg(test)]
mod tests_ui_schema;
#[cfg(test)]
mod tests_ui_roots;
#[cfg(test)]
mod tests_ui_file;
