// Liaisons déclaratives (`bind: user.name` hors d'un gabarit `item:`) :
// chaque chemin commence par une racine, reliée à un objet Ocara par
// `ui.bind("user", user)`. Les propriétés liées sont retirées du nœud (le
// moteur ne lit `bind` que dans une liste) et rangées dans l'arbre :
//
//   "roots": { "user": {
//       "paths": ["name", "address.city"],
//       "nodes": [{ "id": "name", "prop": "value", "path": 0, "kind": "text" }] } }
//
// `kind` (`text`, `bool`, `number`) est vérifié par la sema contre le type du
// champ. Un nœud lié sans `id` en reçoit un (`@user.name`), qui ne peut pas
// entrer en conflit avec un `id` écrit (identifiant).

use std::collections::BTreeMap;

use serde_json::{json, Map, Value as Json};

use super::schema::{self, Ty};
use super::values::is_ident;
use super::yaml::Node;
use super::Checker;

/// Nœud lié : (`id`, propriété, chemin sans la racine, nature de la valeur).
pub(super) type Bound = (String, String, String, &'static str);

/// Propriété liée par `bind` pour un type de nœud.
fn bind_prop(ty: &str) -> &'static str {
    match ty {
        "image" => "src",
        "checkbox" | "switch" => "checked",
        _ => "value",
    }
}

/// Nature de la valeur d'une propriété liable.
fn kind_of(ty: Ty) -> Option<&'static str> {
    match ty {
        Ty::Text | Ty::Color | Ty::Icon | Ty::Enum(_) | Ty::StyleRef => Some("text"),
        Ty::Bool => Some("bool"),
        Ty::Int(..) | Ty::Number(..) => Some("number"),
        _ => None,
    }
}

/// `racine.champ[.champ…]` : chemin lié hors d'une liste.
pub(super) fn is_root_path(path: &str) -> bool {
    path.split('.').count() >= 2 && path.split('.').all(is_ident)
}

impl Checker {
    /// Hors d'un gabarit `item:` : retire `bind`/`binds` du nœud `out` (de
    /// type `ty`) et les range dans les racines ; donne un `id` au nœud.
    pub(super) fn collect_bindings(&mut self, node: &Node, ty: &str, out: &mut Map<String, Json>) {
        let mut pairs: Vec<(String, String)> = Vec::new();
        if let Some(Json::String(path)) = out.remove("bind") {
            pairs.push((bind_prop(ty).to_string(), path));
        }
        if let Some(Json::Object(binds)) = out.remove("binds") {
            pairs.extend(binds.into_iter().filter_map(|(prop, path)| Some((prop, path.as_str()?.to_string()))));
        }
        if pairs.is_empty() {
            return;
        }
        let id = match out.get("id").and_then(Json::as_str) {
            Some(id) => id.to_string(),
            None => {
                let id = self.generated_id(&pairs[0].1);
                self.ids.insert(id.clone(), (ty.to_string(), node.line));
                out.insert("id".into(), Json::String(id.clone()));
                id
            }
        };
        for (prop, path) in pairs {
            let Some(kind) = schema::prop_type(ty, &prop).and_then(kind_of) else {
                self.fail::<()>(node, format!("'{ty}' : la propriété '{prop}' ne peut pas être liée à un champ"));
                continue;
            };
            let (root, rest) = path.split_once('.').expect("chemin de racine vérifié");
            self.bindings.entry(root.to_string()).or_default().push((id.clone(), prop, rest.to_string(), kind));
        }
    }

    /// Champ `required`, bouton `submit`, champ à `onSubmit` : leur état et
    /// leurs actions passent par leur `id` (runtime_ui/src/form.rs) ; sans
    /// `id` écrit, `@type` (`@input`, `@button#2`…).
    pub(super) fn form_id(&mut self, node: &Node, ty: &str, out: &mut Map<String, Json>) {
        let flagged = |key: &str| out.get(key) == Some(&Json::Bool(true));
        if out.contains_key("id") || !(flagged("required") || flagged("submit") || out.contains_key("onSubmit")) {
            return;
        }
        let id = self.generated_id(ty);
        self.ids.insert(id.clone(), (ty.to_string(), node.line));
        out.insert("id".into(), Json::String(id));
    }

    /// `id` donné à un nœud qui n'en a pas : `@nom`, puis `@nom#2`…
    fn generated_id(&self, path: &str) -> String {
        let base = format!("@{path}");
        (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}#{n}") })
            .find(|id| !self.ids.contains_key(id))
            .expect("suite infinie")
    }
}

/// Table `roots` de l'arbre (voir l'en-tête).
pub(super) fn roots_json(bindings: &BTreeMap<String, Vec<Bound>>) -> Json {
    let mut roots = Map::new();
    for (root, bound) in bindings {
        let mut paths: Vec<&str> = Vec::new();
        let nodes: Vec<Json> = bound.iter().map(|(id, prop, path, kind)| {
            let index = paths.iter().position(|p| p == path).unwrap_or_else(|| {
                paths.push(path);
                paths.len() - 1
            });
            json!({ "id": id, "prop": prop, "path": index, "kind": kind })
        }).collect();
        roots.insert(root.clone(), json!({ "paths": paths, "nodes": nodes }));
    }
    Json::Object(roots)
}

/// Ajoute les racines d'une variante de taille à celles de l'arbre de base
/// (chemins réunis, nœuds sans doublon).
pub(super) fn merge_roots(base: &mut Map<String, Json>, variant: &Json) {
    let Some(variant) = variant.as_object() else { return };
    for (root, spec) in variant {
        let target = base.entry(root.clone()).or_insert_with(|| json!({ "paths": [], "nodes": [] }));
        let empty = Vec::new();
        let paths = spec["paths"].as_array().unwrap_or(&empty);
        for node in spec["nodes"].as_array().unwrap_or(&empty) {
            let Some(path) = node["path"].as_u64().and_then(|i| paths.get(i as usize)) else { continue };
            let known = target["paths"].as_array().expect("paths").iter().position(|p| p == path);
            let index = known.unwrap_or_else(|| {
                let list = target["paths"].as_array_mut().expect("paths");
                list.push(path.clone());
                list.len() - 1
            });
            let mut node = node.clone();
            node["path"] = json!(index);
            let nodes = target["nodes"].as_array_mut().expect("nodes");
            if !nodes.iter().any(|n| n["id"] == node["id"] && n["prop"] == node["prop"]) {
                nodes.push(node);
            }
        }
    }
}
