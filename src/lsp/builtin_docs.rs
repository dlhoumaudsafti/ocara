//! Catalogue des classes builtin `ocara.*` et de leur documentation
//! (`docs/builtins/*.md`), généré par
//! `tools/highlight/vsode/scripts/generate-builtins-data.py`.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

const DATA: &str = include_str!("../../tools/highlight/vsode/data/builtins-data.json");

pub struct BuiltinMethod {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub returns: String,
    pub is_static: bool,
    pub doc: Option<String>,
    pub doc_file: String,
    pub doc_heading: Option<String>,
}

#[derive(Default)]
pub struct BuiltinClass {
    pub methods: Vec<BuiltinMethod>,
    pub consts: Vec<(String, String)>,
}

impl BuiltinMethod {
    /// `Classe::m(a:T): R` (statique) ou `Classe.m(a:T): R`.
    pub fn signature(&self, class: &str) -> String {
        let sep = if self.is_static { "::" } else { "." };
        let params: Vec<String> = self.params.iter().map(|(n, t)| format!("{}:{}", n, t)).collect();
        format!("{}{}{}({}): {}", class, sep, self.name, params.join(", "), self.returns)
    }
}

fn catalog() -> &'static HashMap<String, BuiltinClass> {
    static CATALOG: OnceLock<HashMap<String, BuiltinClass>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut out = HashMap::new();
        let Ok(Value::Array(classes)) = serde_json::from_str::<Value>(DATA) else { return out };
        for class in &classes {
            let Some(class_name) = class["name"].as_str() else { continue };
            let text = |v: &Value| v.as_str().unwrap_or("").to_string();
            let methods = class["methods"].as_array().into_iter().flatten().map(|m| BuiltinMethod {
                name: text(&m["name"]),
                params: m["params"].as_array().into_iter().flatten().map(|p| (text(&p["name"]), text(&p["type"]))).collect(),
                returns: m["returns"].as_str().unwrap_or("void").to_string(),
                is_static: m["static"].as_bool().unwrap_or(false),
                doc: m["doc"].as_str().map(str::to_string),
                doc_file: m["docFile"].as_str().map(str::to_string).unwrap_or_else(|| format!("{}.md", class_name)),
                doc_heading: m["docHeading"].as_str().map(str::to_string),
            }).collect();
            let consts = class["consts"].as_array().into_iter().flatten().map(|c| (text(&c["name"]), text(&c["type"]))).collect();
            out.insert(class_name.to_string(), BuiltinClass { methods, consts });
        }
        out
    })
}

pub fn class(name: &str) -> Option<&'static BuiltinClass> {
    catalog().get(name)
}

pub fn method(class_name: &str, name: &str) -> Option<&'static BuiltinMethod> {
    class(class_name)?.methods.iter().find(|m| m.name == name)
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

pub fn percent_encode(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{:02X}", b),
    }).collect()
}
