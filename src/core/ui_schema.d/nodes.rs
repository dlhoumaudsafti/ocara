// Nœuds d'interface du `.oc.ui` : type, propriétés permises et obligatoires,
// `id` uniques, surcharges `tauri:`/`android:`, gabarit `item:` des listes,
// menus.

use serde_json::{Map, Value as Json};

use super::schema::{self, Ty};
use super::values::{is_ident, suggest, text};
use super::yaml::{Kind, Node};
use super::Checker;

/// Propriétés qu'une surcharge de plateforme ne peut pas changer.
const NOT_OVERRIDABLE: &[&str] = &["type", "id", "children", "item", "tauri", "android"];

impl Checker {
    /// Valeur de n'importe quel `Ty`, composés compris.
    pub(super) fn value(&mut self, node: &Node, ty: Ty, what: &str) -> Option<Json> {
        match ty {
            Ty::Node => self.node(node),
            Ty::Children => match &node.kind {
                Kind::Seq(items) => Some(Json::Array(items.iter().filter_map(|item| self.node(item)).collect())),
                _ => self.fail(node, format!("{what} : liste de nœuds attendue")),
            },
            Ty::Menu => self.menu(node, what),
            Ty::Options => self.options(node, what),
            Ty::ActionMap => {
                let mut out = Map::new();
                for (key, name, value) in self.entries(node, what)? {
                    if !is_ident(name) {
                        self.fail::<()>(key, format!("{what} : nom '{name}' invalide (lettres, chiffres, '_')"));
                    } else if let Some(action) = self.simple(value, Ty::Action, &format!("{what}.{name}")) {
                        out.insert(name.to_string(), action);
                    }
                }
                Some(Json::Object(out))
            }
            Ty::States => self.state_styles(node, what),
            Ty::Shadow => self.shadow(node, what),
            Ty::Transition => self.transition(node, what),
            Ty::Animation => self.animation_ref(node, what),
            Ty::Override | Ty::Binds => unreachable!("contrôlé par `node`"),
            simple => self.simple(node, simple, what),
        }
    }

    /// Nœud d'interface : table dont `type` est un type connu.
    pub(super) fn node(&mut self, node: &Node) -> Option<Json> {
        let entries = self.entries(node, "nœud")?;
        let Some(&(_, _, type_node)) = entries.iter().find(|(_, k, _)| *k == "type") else {
            return self.fail(node, format!("nœud sans 'type' (types : {})", schema::NODE_TYPES.join(", ")));
        };
        let ty = text(type_node).unwrap_or_default();
        if schema::node_props(ty).is_none() {
            if self.component_names().contains(&ty) {
                return self.instance(node, ty);
            }
            let mut known: Vec<&str> = schema::NODE_TYPES.to_vec();
            known.extend(self.component_names());
            return self.fail(type_node, format!("type de nœud '{ty}' inconnu{}", suggest(ty, &known)));
        }
        let mut out = Map::new();
        out.insert("type".into(), Json::String(ty.to_string()));
        for &(key, name, value) in &entries {
            let what = format!("propriété '{name}' de '{ty}'");
            match schema::prop_type(ty, name) {
                _ if name == "type" => {}
                None => {
                    let known = known_props(ty);
                    self.fail::<()>(key, format!("propriété '{name}' inconnue pour '{ty}'{}", suggest(name, &known)));
                }
                Some(Ty::Id) => {
                    if let Some(id) = self.id(value, ty) {
                        out.insert("id".into(), id);
                    }
                }
                Some(Ty::Binds) => {
                    if let Some(binds) = self.binds(value, ty) {
                        out.insert(name.to_string(), binds);
                    }
                }
                Some(Ty::Override) => {
                    if let Some(over) = self.node_override(value, ty, name) {
                        out.insert(name.to_string(), over);
                    }
                }
                Some(Ty::Node) => {
                    self.in_item += 1;
                    let item = self.node(value);
                    self.in_item -= 1;
                    if let Some(item) = item {
                        out.insert(name.to_string(), item);
                    }
                }
                Some(prop) => {
                    if let Some(v) = self.value(value, prop, &what) {
                        out.insert(name.to_string(), v);
                    }
                }
            }
        }
        if self.in_item == 0 {
            self.collect_bindings(node, ty, &mut out);
            self.form_id(node, ty, &mut out);
        }
        let written: Vec<&str> = entries.iter().map(|&(_, name, _)| name).collect();
        self.node_rules(node, ty, &written, &out);
        if ty == "image" {
            self.embed_image(node, &mut out);
        }
        Some(Json::Object(out))
    }

    /// Règles qui portent sur plusieurs propriétés à la fois.
    /// `written` : clés écrites dans le fichier (une propriété obligatoire
    /// présente mais invalide n'est signalée qu'une fois).
    fn node_rules(&mut self, node: &Node, ty: &str, written: &[&str], props: &Map<String, Json>) {
        for required in schema::required_props(ty) {
            let names: Vec<&str> = required.split('|').collect();
            if !names.iter().any(|name| written.contains(name)) {
                let quoted: Vec<String> = names.iter().map(|n| format!("'{n}'")).collect();
                self.fail::<()>(node, format!("'{ty}' : propriété {} obligatoire", quoted.join(" ou ")));
            }
        }
        if ty == "webview" {
            let sources: Vec<&str> = ["url", "file", "html"].into_iter().filter(|k| props.contains_key(*k)).collect();
            if sources.len() > 1 {
                self.fail::<()>(node, format!("'webview' : une seule source parmi 'url', 'file', 'html' (ici : {})", sources.join(", ")));
            }
        }
        if ty == "list" {
            // Options de grille : seulement avec `display: grid`.
            if props.get("display").and_then(Json::as_str) != Some("grid") {
                for key in ["itemWidth", "minColumns", "maxColumns", "direction"] {
                    if written.contains(&key) {
                        self.fail::<()>(node, format!("'list' : '{key}' n'a de sens qu'avec 'display: grid'"));
                    }
                }
            }
            let bound = |k: &str| props.get(k).and_then(Json::as_i64);
            if let (Some(min), Some(max)) = (bound("minColumns"), bound("maxColumns")) {
                if min > max {
                    self.fail::<()>(node, format!("'list' : 'minColumns' ({min}) doit être inférieur ou égal à 'maxColumns' ({max})"));
                }
            }
        }
        if written.contains(&"onInvalid") && props.get("submit") != Some(&Json::Bool(true)) {
            self.fail::<()>(node, format!("'{ty}' : 'onInvalid' n'a de sens qu'avec 'submit: true'"));
        }
        if ty == "slider" {
            let bound = |k: &str| props.get(k).and_then(Json::as_f64);
            if let (Some(min), Some(max)) = (bound("min"), bound("max")) {
                if min >= max {
                    self.fail::<()>(node, format!("'slider' : 'min' ({min}) doit être inférieur à 'max' ({max})"));
                }
            }
        }
    }

    /// `id` d'un nœud : identifiant, unique dans le fichier.
    fn id(&mut self, value: &Node, ty: &str) -> Option<Json> {
        // `a.b` : `id` interne d'un composant posé avec l'`id` `a`.
        let id = text(value).filter(|s| s.split('.').all(is_ident));
        let Some(id) = id else {
            return self.fail(value, "'id' : identifiant attendu (lettres, chiffres, '_', sans chiffre en tête)".to_string());
        };
        if let Some((_, line)) = self.ids.get(id) {
            let line = *line;
            return self.fail(value, format!("id '{id}' déjà utilisé ligne {line}"));
        }
        self.ids.insert(id.to_string(), (ty.to_string(), value.line));
        Some(Json::String(id.to_string()))
    }

    /// `binds` : chaque propriété (du type du nœud) lit un champ de l'élément
    /// de liste ; hors d'une liste, un chemin `racine.champ` (roots.rs).
    fn binds(&mut self, value: &Node, ty: &str) -> Option<Json> {
        let mut out = Map::new();
        for (key, prop, field) in self.entries(value, "binds")? {
            let known = schema::prop_type(ty, prop).is_some() && !NOT_OVERRIDABLE.contains(&prop) && prop != "binds";
            if !known {
                let props = known_props(ty);
                self.fail::<()>(key, format!("'binds' : propriété '{prop}' inconnue pour '{ty}'{}", suggest(prop, &props)));
                continue;
            }
            match text(field) {
                Some(name) if self.in_item == 0 && !super::roots::is_root_path(name) => {
                    self.fail::<()>(field, format!("'binds.{prop}' : hors d'une liste, 'racine.champ' attendu (racine reliée par ui.bind)"));
                }
                Some(name) if name.split('.').all(is_ident) => {
                    out.insert(prop.to_string(), Json::String(name.to_string()));
                }
                _ => {
                    self.fail::<()>(field, format!("'binds.{prop}' : nom de champ attendu"));
                }
            }
        }
        Some(Json::Object(out))
    }

    /// `tauri:`/`android:` d'un nœud : propriétés du même type.
    fn node_override(&mut self, value: &Node, ty: &str, platform: &str) -> Option<Json> {
        let mut out = Map::new();
        for (key, name, v) in self.entries(value, platform)? {
            match schema::prop_type(ty, name) {
                Some(_) if NOT_OVERRIDABLE.contains(&name) => {
                    self.fail::<()>(key, format!("'{platform}' : '{name}' ne se surcharge pas par plateforme"));
                }
                Some(prop) => {
                    if let Some(v) = self.value(v, prop, &format!("propriété '{platform}.{name}' de '{ty}'")) {
                        out.insert(name.to_string(), v);
                    }
                }
                None => {
                    let known = known_props(ty);
                    self.fail::<()>(key, format!("propriété '{name}' inconnue pour '{ty}' (dans '{platform}'){}", suggest(name, &known)));
                }
            }
        }
        Some(Json::Object(out))
    }

    /// Table de clés connues (`window`, écran, entrée de menu, style…).
    pub(super) fn table(&mut self, node: &Node, what: &str, props: &[(&str, Ty)]) -> Option<Map<String, Json>> {
        let mut out = Map::new();
        for (key, name, value) in self.entries(node, what)? {
            match props.iter().find(|(n, _)| *n == name) {
                Some(&(_, ty)) => {
                    if let Some(v) = self.value(value, ty, &format!("'{what}.{name}'")) {
                        out.insert(name.to_string(), v);
                    }
                }
                None => {
                    let known: Vec<&str> = props.iter().map(|(n, _)| *n).collect();
                    self.fail::<()>(key, format!("clé '{name}' inconnue dans '{what}'{}", suggest(name, &known)));
                }
            }
        }
        Some(out)
    }

    fn menu(&mut self, node: &Node, what: &str) -> Option<Json> {
        let Kind::Seq(items) = &node.kind else {
            return self.fail(node, format!("{what} : liste d'entrées de menu attendue"));
        };
        let mut out = Vec::new();
        for item in items {
            let Some(entry) = self.table(item, "menu", schema::MENU_PROPS) else { continue };
            let separator = entry.get("separator").and_then(Json::as_bool).unwrap_or(false);
            if !separator && !entry.contains_key("text") {
                self.fail::<()>(item, "entrée de menu sans 'text' (ou 'separator: true')".to_string());
            }
            out.push(Json::Object(entry));
        }
        Some(Json::Array(out))
    }

    /// Choix de `radio`/`select`, normalisés en `{ value, text }`.
    fn options(&mut self, node: &Node, what: &str) -> Option<Json> {
        let Kind::Seq(items) = &node.kind else {
            return self.fail(node, format!("{what} : liste de choix attendue"));
        };
        let props: &[(&str, Ty)] = &[("value", Ty::Text), ("text", Ty::Text)];
        let mut out = Vec::new();
        for item in items {
            let option = match &item.kind {
                Kind::Map(_) => {
                    let Some(entry) = self.table(item, "options", props) else { continue };
                    match (entry.get("value"), entry.get("text")) {
                        (Some(value), text) => serde_json::json!({ "value": value, "text": text.unwrap_or(value) }),
                        (None, _) => {
                            self.fail::<()>(item, "choix sans 'value'".to_string());
                            continue;
                        }
                    }
                }
                _ => match self.simple(item, Ty::Text, what) {
                    Some(value) => serde_json::json!({ "value": value, "text": value }),
                    None => continue,
                },
            };
            out.push(option);
        }
        Some(Json::Array(out))
    }
}

fn known_props(ty: &str) -> Vec<&'static str> {
    schema::node_props(ty).unwrap_or_default().iter()
        .chain(schema::COMMON_PROPS)
        .chain(schema::STYLE_PROPS)
        .map(|(n, _)| *n)
        .collect()
}
