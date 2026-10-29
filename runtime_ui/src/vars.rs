//! Variables `${nom}` des textes d'un `.oc.ui`, remplies par
//! `use UI(path, vars)` et `ui.setVars(vars)` (textes de langue). Une
//! variable sans valeur reste écrite telle quelle (`${nom}`), visible à
//! l'écran ; `$${` écrit un `${` littéral.

use serde_json::{Map, Value};

/// Clés de l'arbre qui ne sont pas des textes d'interface.
const NOT_TEXT: &[&str] = &["icons", "source", "ids", "actions", "vars", "sizes"];

fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn substitute(text: &str, vars: &Map<String, Value>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("${") {
        if rest[..at].ends_with('$') {
            out.push_str(&rest[..at - 1]);
            out.push_str("${");
            rest = &rest[at + 2..];
            continue;
        }
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        match after.find('}') {
            Some(end) => {
                let name = &after[..end];
                match vars.get(name) {
                    Some(value) => out.push_str(&text_of(value)),
                    None => out.push_str(&rest[at..at + 3 + end]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[at..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Arbre avec ses textes remplis (les clés de `NOT_TEXT` ne sont sautées
/// qu'à la racine de l'arbre).
pub fn resolve_tree(tree: &Value, vars: &Map<String, Value>) -> Value {
    match tree {
        Value::Object(map) => Value::Object(map.iter()
            .map(|(k, v)| (k.clone(), if NOT_TEXT.contains(&k.as_str()) { v.clone() } else { resolve(v, vars) }))
            .collect()),
        other => resolve(other, vars),
    }
}

/// Valeur avec ses textes remplis (valeur posée par `ui.set`, message).
pub fn resolve(value: &Value, vars: &Map<String, Value>) -> Value {
    match value {
        Value::String(text) if text.contains("${") => Value::String(substitute(text, vars)),
        Value::Array(items) => Value::Array(items.iter().map(|v| resolve(v, vars)).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), resolve(v, vars))).collect()),
        other => other.clone(),
    }
}

/// Variables utilisées par l'arbre sans valeur dans `vars`.
pub fn missing(tree: &Value, vars: &Map<String, Value>) -> Vec<String> {
    tree["vars"].as_array().into_iter().flatten()
        .filter_map(Value::as_str)
        .filter(|name| !vars.contains_key(*name))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn substitution() {
        let vars = json!({ "title": "Garage", "n": 3 }).as_object().unwrap().clone();
        assert_eq!(substitute("${title} (${n})", &vars), "Garage (3)");
        assert_eq!(substitute("prix : $${x}", &vars), "prix : ${x}");
        assert_eq!(substitute("${absent}!", &vars), "${absent}!");
        assert_eq!(substitute("ouvert ${", &vars), "ouvert ${");
        let tree = json!({ "screens": { "a": { "title": "${title}" } }, "icons": { "x": "${title}" } });
        let resolved = resolve_tree(&tree, &vars);
        assert_eq!(resolved["screens"]["a"]["title"], "Garage");
        assert_eq!(resolved["icons"]["x"], "${title}");
        // Hors de la racine, `source` est une donnée comme une autre.
        let row = resolve(&json!({ "source": "${title}" }), &vars);
        assert_eq!(row["source"], "Garage");
    }
}
