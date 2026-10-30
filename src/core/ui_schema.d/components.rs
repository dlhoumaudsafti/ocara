// Composants d'interface déclarés dans un `.oc.ui` (`components:`), développés
// à la compilation : un nœud dont le `type` nomme un composant est remplacé
// par le `layout` du composant, ses `$param` remplacés par les valeurs de
// l'instance. Les moteurs de rendu (desktop, Android) ne voient que des
// nœuds ordinaires, validés comme les autres.
//
//   components:
//     carCard:
//       params:
//         title: text                                   // obligatoire
//         photo: { type: text, required: false }        // facultatif
//         onOpen: action
//       layout:
//         type: card
//         onClick: $onOpen
//         children:
//           - { type: image, src: $photo }              // retiré sans photo
//           - { type: text, value: "Voiture : $title" }
//     includes: [ components/other.oc.ui ]
//
//   - { type: carCard, id: best, title: "Clio", onOpen: open_car }
//
// L'`id` de l'instance va sur la racine du composant et préfixe les `id`
// internes (`best.photo`) : un composant se pose plusieurs fois. Dans le
// gabarit `item` d'une liste, un paramètre peut lire un champ de l'élément
// (`title: { bind: model }`), qui devient un `binds` du nœud. Les
// propriétés de l'instance qui ne sont pas des paramètres (`width`,
// `margin`, `visible`…) s'appliquent à la racine.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde_json::Value as Json;

use super::schema::{self, Ty};
use super::values::{is_ident, suggest, text};
use super::yaml::{self, Kind, Node};
use super::Checker;

/// Types de paramètres et `Ty` contrôlé à l'instance.
pub const PARAM_TYPES: &[(&str, Ty)] = &[
    ("text", Ty::Text),
    ("number", Ty::Number(f64::MIN, f64::MAX)),
    ("bool", Ty::Bool),
    ("color", Ty::Color),
    ("icon", Ty::Icon),
    ("action", Ty::Action),
    ("style", Ty::StyleRef),
    ("length", Ty::Length),
];

#[derive(Debug, Clone)]
pub(super) struct Param {
    name: String,
    ty: Ty,
    required: bool,
    default: Option<Node>,
}

#[derive(Debug, Clone)]
pub(super) struct Component {
    params: Vec<Param>,
    layout: Node,
}

impl Checker {
    /// Composants du fichier et des fichiers inclus (`screens.includes`,
    /// `components.includes`), connus avant les nœuds ; leurs déclarations
    /// sont validées ici (situées dans leur fichier).
    pub(super) fn declare_components(&mut self, root: &Node) {
        let file = self.file.clone();
        let mut seen = BTreeSet::new();
        self.collect_components(root, &file, &mut seen);
    }

    fn collect_components(&mut self, root: &Node, file: &Path, seen: &mut BTreeSet<PathBuf>) {
        seen.insert(std::fs::canonicalize(file).unwrap_or(file.to_path_buf()));
        let Kind::Map(pairs) = &root.kind else { return };
        for (key, value) in pairs {
            let section = text(key);
            if !matches!(section, Some("components" | "screens")) {
                continue;
            }
            let Kind::Map(entries) = &value.kind else { continue };
            for (name, body) in entries {
                match text(name) {
                    Some("includes") => self.follow_includes(body, file, seen),
                    Some(_) if section == Some("components") => self.declare(name, body, file),
                    _ => {}
                }
            }
        }
    }

    fn follow_includes(&mut self, list: &Node, file: &Path, seen: &mut BTreeSet<PathBuf>) {
        let Kind::Seq(items) = &list.kind else { return };
        for relative in items.iter().filter_map(text) {
            let path = file.parent().unwrap_or(Path::new("")).join(relative);
            let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());
            if seen.contains(&canonical) {
                continue;
            }
            // Fichier absent, YAML invalide : signalés par la validation des includes.
            let Ok(source) = std::fs::read_to_string(&path) else { continue };
            if let Ok(Some(included)) = yaml::load(&source) {
                self.collect_components(&included, &path, seen);
            }
        }
    }

    /// Déclaration `nom: { params, layout }` lue dans `file`.
    fn declare(&mut self, key: &Node, body: &Node, file: &Path) {
        let origin = Rc::new(file.to_path_buf());
        let key = located(key, &origin);
        let body = located(body, &origin);
        let Some(name) = text(&key).map(str::to_string) else { return };
        if !is_ident(&name) {
            self.fail::<()>(&key, format!("nom de composant '{name}' invalide (lettres, chiffres, '_')"));
            return;
        }
        if schema::node_props(&name).is_some() {
            self.fail::<()>(&key, format!("composant '{name}' : nom d'un type de nœud prédéfini"));
            return;
        }
        if self.components.contains_key(&name) {
            self.fail::<()>(&key, format!("composant '{name}' déjà déclaré"));
            return;
        }
        let Some(entries) = self.entries(&body, &format!("components.{name}")) else { return };
        let mut params = Vec::new();
        let mut layout = None;
        for (k, field, value) in entries {
            match field {
                "params" => params = self.params(value, &name),
                "layout" => layout = Some(value.clone()),
                _ => {
                    self.fail::<()>(k, format!("clé '{field}' inconnue dans le composant '{name}' (clés : params, layout)"));
                }
            }
        }
        let Some(layout) = layout else {
            self.fail::<()>(&body, format!("composant '{name}' sans 'layout'"));
            return;
        };
        let known: Vec<&str> = params.iter().map(|p: &Param| p.name.as_str()).collect();
        for (node, used) in references(&layout) {
            if !known.contains(&used.as_str()) {
                self.fail::<()>(&node, format!("composant '{name}' : '${used}' n'est pas un paramètre{}", suggest(&used, &known)));
            }
        }
        self.components.insert(name, Component { params, layout });
    }

    /// `params:` : `nom: type` ou `nom: { type, required, default }`.
    fn params(&mut self, node: &Node, component: &str) -> Vec<Param> {
        let names: Vec<&str> = PARAM_TYPES.iter().map(|(n, _)| *n).collect();
        let mut out = Vec::new();
        for (key, name, spec) in self.entries(node, &format!("components.{component}.params")).unwrap_or_default() {
            if !is_ident(name) || name == "id" || name == "type" {
                self.fail::<()>(key, format!("paramètre '{name}' invalide (identifiant, ni 'id' ni 'type')"));
                continue;
            }
            let (type_node, required, default) = match &spec.kind {
                Kind::Map(_) => {
                    let fields = self.entries(spec, &format!("paramètre '{name}'")).unwrap_or_default();
                    let get = |f: &str| fields.iter().find(|(_, n, _)| *n == f).map(|&(_, _, v)| v);
                    let required = get("required").and_then(text).map(|t| t != "false").unwrap_or(true);
                    (get("type").unwrap_or(spec), required, get("default").cloned())
                }
                _ => (spec, true, None),
            };
            let Some(&(_, ty)) = text(type_node).and_then(|t| PARAM_TYPES.iter().find(|(n, _)| *n == t)) else {
                let given = text(type_node).unwrap_or("");
                self.fail::<()>(type_node, format!("paramètre '{name}' : type '{given}' inconnu (types : {}){}", names.join(", "), suggest(given, &names)));
                continue;
            };
            out.push(Param { name: name.to_string(), ty, required: required && default.is_none(), default });
        }
        out
    }

    /// Nœud `type: <composant>` : développé puis validé comme un nœud.
    pub(super) fn instance(&mut self, node: &Node, ty: &str) -> Option<Json> {
        let component = self.components.get(ty)?.clone();
        if self.expanding.iter().any(|c| c == ty) {
            let chain = format!("{} → {ty}", self.expanding.join(" → "));
            return self.fail(node, format!("composant '{ty}' utilisé dans lui-même ({chain})"));
        }
        let mut values: HashMap<String, Node> = HashMap::new();
        let mut binds: HashMap<String, String> = HashMap::new();
        let mut extra: Vec<(Node, Node)> = Vec::new();
        let mut id = None;
        for (key, name, value) in self.entries(node, "nœud")? {
            match name {
                "type" => {}
                "id" => match text(value).filter(|s| s.split('.').all(is_ident)) {
                    Some(v) => id = Some(v.to_string()),
                    None => {
                        self.fail::<()>(value, "'id' : identifiant attendu (lettres, chiffres, '_')".to_string());
                    }
                },
                _ => match component.params.iter().find(|p| p.name == name) {
                    Some(param) => match bound_field(value) {
                        Some(field) if self.in_item > 0 => {
                            binds.insert(name.to_string(), field);
                        }
                        Some(_) => {
                            self.fail::<()>(value, format!("'{name}' : 'bind' n'est permis que dans le gabarit 'item' d'une liste"));
                        }
                        None => {
                            if self.value(value, param.ty, &format!("paramètre '{name}' de '{ty}'")).is_some() {
                                values.insert(name.to_string(), value.clone());
                            }
                        }
                    },
                    None => extra.push((key.clone(), value.clone())),
                },
            }
        }
        for param in &component.params {
            if values.contains_key(&param.name) || binds.contains_key(&param.name) {
                continue;
            }
            match &param.default {
                Some(default) => {
                    values.insert(param.name.clone(), default.clone());
                }
                None if param.required => {
                    self.fail::<()>(node, format!("'{ty}' : paramètre '{}' obligatoire", param.name));
                }
                None => {}
            }
        }
        let names: Vec<&str> = component.params.iter().map(|p| p.name.as_str()).collect();
        let mut expanded = substitute(&component.layout, &names, &values, &binds);
        if let Some(id) = &id {
            prefix_ids(&mut expanded, id, true);
        }
        // Propriétés de l'instance hors paramètres : sur la racine (validées pour son type).
        if let Kind::Map(pairs) = &mut expanded.kind {
            for (key, value) in extra {
                pairs.retain(|(k, _)| text(k) != text(&key));
                pairs.push((key, value));
            }
        }
        self.expanding.push(ty.to_string());
        let out = self.node(&expanded);
        self.expanding.pop();
        out
    }

    /// Noms des composants connus (messages d'erreur).
    pub(super) fn component_names(&self) -> Vec<&str> {
        self.components.keys().map(String::as_str).collect()
    }
}

/// Copie de `node` située dans `file` (erreurs de ses nœuds dans ce fichier).
fn located(node: &Node, file: &Rc<PathBuf>) -> Node {
    let mut out = node.clone();
    out.file.get_or_insert_with(|| file.clone());
    out.kind = match &node.kind {
        Kind::Seq(items) => Kind::Seq(items.iter().map(|n| located(n, file)).collect()),
        Kind::Map(pairs) => Kind::Map(pairs.iter().map(|(k, v)| (located(k, file), located(v, file))).collect()),
        scalar => scalar.clone(),
    };
    out
}

/// `{ bind: champ }` : valeur d'un paramètre lue dans l'élément de liste.
fn bound_field(value: &Node) -> Option<String> {
    let Kind::Map(pairs) = &value.kind else { return None };
    match pairs.as_slice() {
        [(key, field)] if text(key) == Some("bind") => text(field).map(str::to_string),
        _ => None,
    }
}

/// `$nom` dans un texte : (début, nom).
fn params_in(text: &str) -> Vec<(usize, String)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && (i == 0 || bytes[i - 1] != b'$') {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end > start && !bytes[start].is_ascii_digit() {
                out.push((i, text[start..end].to_string()));
                i = end;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Nœuds qui citent un `$param`, et le nom cité.
fn references(node: &Node) -> Vec<(Node, String)> {
    match &node.kind {
        Kind::Scalar { text, .. } => params_in(text).into_iter().map(|(_, name)| (node.clone(), name)).collect(),
        Kind::Seq(items) => items.iter().flat_map(references).collect(),
        Kind::Map(pairs) => pairs.iter().flat_map(|(_, v)| references(v)).collect(),
    }
}

/// Valeur entière `$nom` d'un scalaire.
fn whole(node: &Node) -> Option<&str> {
    let Kind::Scalar { text, .. } = &node.kind else { return None };
    text.strip_prefix('$').filter(|n| !n.is_empty() && is_ident(n))
}

fn scalar(text: &str, at: &Node) -> Node {
    Node { kind: Kind::Scalar { text: text.to_string(), plain: false }, line: at.line, col: at.col, file: at.file.clone() }
}

/// `layout` avec les valeurs des paramètres : `$nom` seul est remplacé par
/// la valeur (paramètre absent : la clé est retirée ; lu dans l'élément :
/// `binds`) ; dans un texte, par le texte de la valeur.
fn substitute(node: &Node, params: &[&str], values: &HashMap<String, Node>, binds: &HashMap<String, String>) -> Node {
    match &node.kind {
        Kind::Scalar { text, plain } => {
            if let Some(value) = whole(node).and_then(|n| values.get(n)) {
                return value.clone();
            }
            let mut out = String::new();
            let mut last = 0;
            for (at, name) in params_in(text).into_iter().filter(|(_, n)| params.contains(&n.as_str())) {
                out.push_str(&text[last..at]);
                if let Some(Kind::Scalar { text: value, .. }) = values.get(&name).map(|v| &v.kind) {
                    out.push_str(value);
                }
                last = at + 1 + name.len();
            }
            out.push_str(&text[last..]);
            Node { kind: Kind::Scalar { text: out, plain: *plain }, ..node.clone() }
        }
        Kind::Seq(items) => {
            let kept = items.iter()
                .filter(|item| whole(item).is_none_or(|n| !params.contains(&n) || values.contains_key(n)))
                .map(|item| substitute(item, params, values, binds))
                .collect();
            Node { kind: Kind::Seq(kept), ..node.clone() }
        }
        Kind::Map(pairs) => {
            let mut out = Vec::new();
            let mut bound: Vec<(Node, Node)> = Vec::new();
            for (key, value) in pairs {
                match whole(value).filter(|n| params.contains(n)) {
                    Some(name) if binds.contains_key(name) => bound.push((key.clone(), scalar(&binds[name], value))),
                    Some(name) if !values.contains_key(name) => {}
                    _ => out.push((key.clone(), substitute(value, params, values, binds))),
                }
            }
            if !bound.is_empty() {
                match out.iter_mut().find(|(k, _)| text(k) == Some("binds")) {
                    Some((_, Node { kind: Kind::Map(existing), .. })) => existing.extend(bound),
                    _ => {
                        let at = &pairs[0].0;
                        out.push((scalar("binds", at), Node { kind: Kind::Map(bound), ..at.clone() }));
                    }
                }
            }
            Node { kind: Kind::Map(out), ..node.clone() }
        }
    }
}

/// `id` de l'instance sur la racine, préfixe des `id` internes (`id.interne`).
fn prefix_ids(node: &mut Node, id: &str, root: bool) {
    let Kind::Map(pairs) = &mut node.kind else { return };
    let mut has_id = false;
    for (key, value) in pairs.iter_mut() {
        match text(key) {
            Some("id") => {
                has_id = true;
                if let Kind::Scalar { text: own, .. } = &mut value.kind {
                    *own = if root { id.to_string() } else { format!("{id}.{own}") };
                }
            }
            Some("children") => {
                if let Kind::Seq(items) = &mut value.kind {
                    items.iter_mut().for_each(|child| prefix_ids(child, id, false));
                }
            }
            Some("item") => prefix_ids(value, id, false),
            _ => {}
        }
    }
    if root && !has_id {
        let at = node.clone();
        if let Kind::Map(pairs) = &mut node.kind {
            pairs.push((scalar("id", &at), scalar(id, &at)));
        }
    }
}
