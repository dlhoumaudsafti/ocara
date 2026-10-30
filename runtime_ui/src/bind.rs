//! Liaisons déclaratives (`bind: user.name` hors d'une liste, docs/oc-ui.md) :
//! `ui.bind("user", objet)` relie la racine `user` à un objet Ocara. Le
//! compilateur génère l'accesseur de la classe de l'objet, une closure
//! `(objet, rang, valeur): mixed`, et la liste de ses chemins
//! (`[{ "path": "address.city", "kinds": ["text"] }]`) : `rang` ≥ 0 lit le
//! chemin `rang`, `-rang - 1` y écrit `valeur` (conversion comprise ; une
//! saisie invalide lève `ConvertException` et n'écrit rien). Chaque chemin
//! lié par le `.oc.ui` (`roots`) doit y figurer avec une nature compatible.
//!
//! - Code → interface : après chaque action, chaque chemin lié est relu ; un
//!   nœud dont la valeur a changé depuis sa dernière lecture la reçoit.
//! - Interface → code : une saisie sur un nœud lié est écrite dans l'objet
//!   sur le thread des actions, avant l'action suivante. La valeur relue
//!   devient celle du nœud saisi : ni écho vers le moteur, ni texte écrasé
//!   pendant la frappe quand la saisie est invalide (objet inchangé).

use serde_json::Value;

use crate::ffi::{self, from_json, raise, str_arg, to_json};
use crate::{with, Ui, ERR_ROOT};

/// Champ compté `__kept` de l'objet `UI` : objets et accesseurs liés y sont
/// gardés, pour que le collecteur voie un cycle objet → `ui` → objet.
const KEPT_FIELD: i64 = 1;

#[derive(Clone)]
pub(crate) struct Binding {
    object: i64,
    accessor: i64,
    /// Rang dans l'accesseur de chaque chemin de `roots.<racine>.paths`.
    ranks: Vec<usize>,
}

/// Envoi d'une propriété au moteur de la plateforme (même message qu'un
/// `ui.set`).
pub type SendSet = fn(this: i64, id: &str, prop: &str, value: &Value);

/// Nœuds liés à la racine `root` : (`id`, propriété, rang du chemin).
fn bound_nodes(ui: &Ui, root: &str) -> Vec<(String, String, usize)> {
    ui.tree["roots"][root]["nodes"].as_array().into_iter().flatten().filter_map(|node| Some((
        node["id"].as_str()?.to_string(),
        node["prop"].as_str()?.to_string(),
        node["path"].as_u64()? as usize,
    ))).collect()
}

/// Rang dans l'accesseur de chaque chemin lié à `root`, ou le message de
/// l'`UIException` : chemin absent de la classe, ou nature incompatible.
fn ranks(ui: &Ui, root: &str, accessor_paths: &Value) -> Result<Vec<usize>, String> {
    let Some(spec) = ui.tree["roots"].get(root) else {
        let known: Vec<&str> = ui.tree["roots"].as_object().map(|r| r.keys().map(String::as_str).collect()).unwrap_or_default();
        return Err(format!("racine '{root}' absente des liaisons du .oc.ui (racines : {})", known.join(", ")));
    };
    let entries = accessor_paths.as_array().map(Vec::as_slice).unwrap_or_default();
    let paths: Vec<&str> = spec["paths"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let ranks = paths.iter().map(|path| entries.iter().position(|e| e["path"] == *path)
        .ok_or_else(|| format!("'{root}.{path}' : champ absent ou inaccessible dans l'objet lié")))
        .collect::<Result<Vec<usize>, String>>()?;
    for node in spec["nodes"].as_array().into_iter().flatten() {
        let (Some(path), Some(kind)) = (node["path"].as_u64(), node["kind"].as_str()) else { continue };
        let entry = &entries[ranks[path as usize]];
        if !entry["kinds"].as_array().is_some_and(|k| k.iter().any(|k| k == kind)) {
            return Err(format!("'{root}.{}' : le champ ne convient pas à '{}' du nœud '{}' (valeur {kind})",
                paths[path as usize], node["prop"].as_str().unwrap_or(""), node["id"].as_str().unwrap_or("")));
        }
    }
    Ok(ranks)
}

/// `ui.bind(root, objet)` : remplace l'objet lié à `root` et affiche ses
/// valeurs (thread de l'appelant, qui exécute déjà du code Ocara).
pub fn bind(this: i64, root: &str, object: i64, accessor_paths: &str, accessor: i64) {
    let paths: Value = serde_json::from_str(accessor_paths).unwrap_or(Value::Null);
    let ranks = match with(this, |ui| ranks(ui, root, &paths)) {
        Some(Ok(ranks)) => ranks,
        Some(Err(message)) => raise(ERR_ROOT, message),
        None => return,
    };
    let previous = with(this, |ui| ui.bindings.insert(root.to_string(), Binding { object, accessor, ranks })).flatten();
    unsafe {
        ffi::__rc_keep(this, KEPT_FIELD, object);
        ffi::__rc_keep(this, KEPT_FIELD, accessor);
    }
    if let Some(old) = previous {
        release(this, old);
    }
    sync(this);
}

/// `ui.unbind(root)` : les nœuds gardent leurs dernières valeurs.
pub fn unbind(this: i64, root: &str) {
    if let Some(old) = with(this, |ui| ui.bindings.remove(root)).flatten() {
        release(this, old);
    }
}

fn release(this: i64, binding: Binding) {
    unsafe {
        ffi::__rc_unkeep(this, KEPT_FIELD, binding.object);
        ffi::__rc_unkeep(this, KEPT_FIELD, binding.accessor);
    }
}

/// Appel de l'accesseur ; `None` si une exception l'a interrompu
/// (`ConvertException` d'une saisie invalide : silencieuse).
fn call(binding: &Binding, rank: i64, value: i64) -> Option<i64> {
    let mut out = 0;
    let uncaught = unsafe { ffi::__call_closure3_value(binding.accessor, binding.object, rank, value, &mut out) };
    if uncaught == 0 {
        return Some(out);
    }
    let message = str_arg(uncaught);
    if !message.starts_with("ConvertException") {
        eprintln!("UI : liaison : exception non rattrapée ({message})");
    }
    ffi::release(uncaught);
    None
}

/// Valeur du chemin `path` (rang dans `roots.<racine>.paths`).
fn read(binding: &Binding, path: usize) -> Value {
    let Some(value) = call(binding, binding.ranks[path] as i64, 0) else { return Value::Null };
    let json = to_json(value);
    ffi::release(value);
    json
}

/// Code → interface : relit chaque chemin lié et envoie ce qui a changé.
/// Jamais sous le verrou des interfaces : l'accesseur exécute du code Ocara.
pub(crate) fn sync(this: i64) {
    let bound: Vec<(Binding, Vec<(String, String, usize)>)> = with(this, |ui| {
        ui.bindings.iter().map(|(root, binding)| (binding.clone(), bound_nodes(ui, root))).collect()
    }).unwrap_or_default();
    for (binding, nodes) in bound {
        let mut values: Vec<Option<Value>> = Vec::new();
        for (id, prop, path) in nodes {
            if values.len() <= path {
                values.resize(path + 1, None);
            }
            let value = values[path].get_or_insert_with(|| read(&binding, path)).clone();
            let changed = with(this, |ui| {
                let key = (id.clone(), prop.clone());
                if ui.seen.get(&key) == Some(&value) {
                    return false;
                }
                ui.seen.insert(key, value.clone());
                ui.state.entry(id.clone()).or_default().insert(prop.clone(), value.clone());
                true
            }).unwrap_or(false);
            if let (true, Some(send)) = (changed, crate::SEND_SET.get()) {
                send(this, &id, &prop, &value);
            }
        }
    }
}

/// Le nœud `id`/`prop` est-il lié ? Sa racine et le rang de son chemin.
pub(crate) fn target(this: i64, id: &str, prop: &str) -> Option<(Binding, usize)> {
    with(this, |ui| {
        ui.bindings.iter().find_map(|(root, binding)| {
            bound_nodes(ui, root).into_iter()
                .find(|(i, p, _)| i == id && p == prop)
                .map(|(_, _, path)| (binding.clone(), path))
        })
    }).flatten()
}

/// Interface → code (thread des actions) : écrit la saisie dans l'objet,
/// puis propage aux autres nœuds liés au même chemin.
pub(crate) fn write(this: i64, id: &str, prop: &str, value: Value) {
    let Some((binding, path)) = target(this, id, prop) else { return };
    let ocara = from_json(&value);
    if let Some(result) = call(&binding, -(binding.ranks[path] as i64) - 1, ocara) {
        ffi::release(result);
    }
    ffi::release(ocara);
    let now = read(&binding, path);
    with(this, |ui| ui.seen.insert((id.to_string(), prop.to_string()), now));
    sync(this);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{json, Map};

    use super::ranks;
    use crate::Ui;

    /// Interface de test (sans `init`, qui appelle le runtime Ocara).
    fn ui() -> Ui {
        let tree = json!({ "roots": { "user": {
            "paths": ["name", "age"],
            "nodes": [
                { "id": "name", "prop": "value", "path": 0, "kind": "text" },
                { "id": "age", "prop": "value", "path": 1, "kind": "number" }
            ]
        } } });
        Ui {
            raw: tree.clone(),
            vars: Map::new(),
            tree,
            nodes: HashMap::new(),
            state: HashMap::new(),
            handlers: HashMap::new(),
            size_class: String::new(),
            bindings: HashMap::new(),
            seen: HashMap::new(),
        }
    }

    #[test]
    fn paths_are_found_in_the_accessor_whatever_their_order() {
        let accessor = json!([
            { "path": "age", "kinds": ["text", "number"] },
            { "path": "email", "kinds": ["text"] },
            { "path": "name", "kinds": ["text"] }
        ]);
        assert_eq!(ranks(&ui(), "user", &accessor), Ok(vec![2, 0]));
    }

    #[test]
    fn unknown_root_missing_path_and_wrong_kind_are_errors() {
        let ui = ui();
        assert!(ranks(&ui, "car", &json!([])).unwrap_err().contains("racine 'car'"));
        let missing = ranks(&ui, "user", &json!([{ "path": "name", "kinds": ["text"] }]));
        assert!(missing.unwrap_err().contains("'user.age'"));
        let wrong = ranks(&ui, "user", &json!([
            { "path": "name", "kinds": ["text"] },
            { "path": "age", "kinds": ["text"] }
        ]));
        assert!(wrong.unwrap_err().contains("valeur number"));
    }
}
