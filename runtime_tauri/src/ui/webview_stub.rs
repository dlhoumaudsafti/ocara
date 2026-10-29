// Composant `webview` d'ocara.UI hors Linux : pas encore de vue posée (la
// place du nœud reste vide). Voir webview.rs et docs/builtins/UI.md.

use serde_json::Value;

pub(crate) fn place(_this: i64, _message: &Value) {}
pub(crate) fn apply(_this: i64, _id: &str, _prop: &str) {}
pub(crate) fn post_message(_this: i64, _id: &str, _data: &Value) {}
pub(crate) fn eval(_this: i64, _id: &str, _script: &str) -> String {
    String::new()
}
pub(crate) fn navigate(_this: i64, _id: &str, _step: &'static str) {}
pub(crate) fn release(_this: i64) {}
