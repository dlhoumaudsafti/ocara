//! Catalogue des classes builtin `ocara.*` et de leur documentation
//! (`docs/builtins/*.md`), généré par
//! `tools/highlight/vsode/scripts/generate-builtins-data.py`.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

const DATA: &str = include_str!("../../tools/highlight/vsode/data/builtins-data.json");

pub struct BuiltinMethod {
    pub signature: String,
    pub doc: Option<String>,
    pub doc_file: String,
    pub doc_heading: Option<String>,
}

fn catalog() -> &'static HashMap<(String, String), BuiltinMethod> {
    static CATALOG: OnceLock<HashMap<(String, String), BuiltinMethod>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut out = HashMap::new();
        let Ok(Value::Array(classes)) = serde_json::from_str::<Value>(DATA) else { return out };
        for class in &classes {
            let Some(class_name) = class["name"].as_str() else { continue };
            for m in class["methods"].as_array().into_iter().flatten() {
                let Some(name) = m["name"].as_str() else { continue };
                let params: Vec<String> = m["params"].as_array().into_iter().flatten()
                    .map(|p| format!("{}:{}", p["name"].as_str().unwrap_or(""), p["type"].as_str().unwrap_or("")))
                    .collect();
                let sep = if m["static"].as_bool().unwrap_or(false) { "::" } else { "." };
                out.insert((class_name.to_string(), name.to_string()), BuiltinMethod {
                    signature: format!("{}{}{}({}): {}", class_name, sep, name, params.join(", "), m["returns"].as_str().unwrap_or("void")),
                    doc: m["doc"].as_str().map(str::to_string),
                    doc_file: m["docFile"].as_str().map(str::to_string).unwrap_or_else(|| format!("{}.md", class_name)),
                    doc_heading: m["docHeading"].as_str().map(str::to_string),
                });
            }
        }
        out
    })
}

pub fn method(class: &str, name: &str) -> Option<&'static BuiltinMethod> {
    catalog().get(&(class.to_string(), name.to_string()))
}

pub fn is_builtin_class(name: &str) -> bool {
    crate::core::analysis::OCARA_BUILTINS.contains(&name)
}

/// Lien vers la documentation : `ocara-doc:<chemin>#<titre>`, que le client
/// traduit en ouverture de la documentation embarquée.
pub fn doc_link(file: &str, heading: Option<&str>) -> String {
    let anchor = heading.map(|h| format!("#{}", percent_encode(h))).unwrap_or_default();
    format!("[📖 docs/builtins/{}](ocara-doc:builtins/{}{})", file, file, anchor)
}

fn percent_encode(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{:02X}", b),
    }).collect()
}
