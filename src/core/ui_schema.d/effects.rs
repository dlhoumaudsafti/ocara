// Styles d'état, ombres, transitions et animations (docs/oc-ui.md, « Styles
// d'état et effets », « Animations ») :
//
//   styles:
//     bouton:
//       background: "#1565c0"
//       states:
//         hover: { background: "#1e88e5" }    // table d'état
//       transition: { duration: 150, properties: [background] }
//       shadow: { y: 2, blur: 6, color: "rgba(0, 0, 0, 0.3)" }
//   animations:
//     pulse:
//       duration: 800
//       iterations: infinite
//       keyframes:
//         0%: { opacity: 1 }
//         100%: { opacity: 0.6, scale: 1.05 }
//
// Dans l'arbre : `shadow` est une liste d'ombres complètes, `animations` une
// table nom → `{ duration, delay, easing, iterations, direction, fill,
// keyframes: [{ at, …propriétés }] }` (`at` en %, trié), `animation` d'un
// nœud `{ name, on }`.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value as Json};

use super::schema::{self, Ty};
use super::values::{is_ident, suggest, text};
use super::yaml::{Kind, Node};
use super::Checker;

/// États d'un nœud dont une table de style change l'apparence.
pub const STATES: &[&str] = &["hover", "focus", "active", "disabled", "invalid", "checked", "visited"];

/// Déclenchement d'une animation posée sur un nœud.
pub const TRIGGERS: &[&str] = &["appear", "hover", "active", "focus"];

pub const EASINGS: &[&str] = &["linear", "ease", "ease-in", "ease-out", "ease-in-out"];

/// Propriétés d'une étape (`keyframes`) : celles qu'un moteur sait animer.
pub const KEYFRAME_PROPS: &[(&str, Ty)] = &[
    ("opacity", Ty::Number(0.0, 1.0)),
    ("scale", Ty::Number(0.0, 100.0)),
    ("rotate", Ty::Number(-36_000.0, 36_000.0)),
    ("translateX", Ty::Number(-100_000.0, 100_000.0)),
    ("translateY", Ty::Number(-100_000.0, 100_000.0)),
    ("background", Ty::Color),
    ("color", Ty::Color),
    ("blur", Ty::Number(0.0, 1000.0)),
    ("width", Ty::Number(0.0, 100_000.0)),
    ("height", Ty::Number(0.0, 100_000.0)),
];

pub const SHADOW_PROPS: &[(&str, Ty)] = &[
    ("x", Ty::Number(-1000.0, 1000.0)),
    ("y", Ty::Number(-1000.0, 1000.0)),
    ("blur", Ty::Number(0.0, 1000.0)),
    ("spread", Ty::Number(-1000.0, 1000.0)),
    ("color", Ty::Color),
    ("inset", Ty::Bool),
];

pub const TRANSITION_PROPS: &[(&str, Ty)] = &[
    ("duration", Ty::Int(0, 60_000)),
    ("delay", Ty::Int(0, 60_000)),
    ("easing", Ty::Enum(EASINGS)),
    ("properties", Ty::Strings),
];

pub const ANIMATION_PROPS: &[(&str, Ty)] = &[
    ("duration", Ty::Int(0, 600_000)),
    ("delay", Ty::Int(0, 600_000)),
    ("easing", Ty::Enum(EASINGS)),
    ("direction", Ty::Enum(&["normal", "reverse", "alternate", "alternate-reverse"])),
    ("fill", Ty::Enum(&["none", "forwards", "backwards", "both"])),
];

/// Propriétés de style d'une table d'état : toutes, sauf `states`.
pub fn state_props() -> Vec<(&'static str, Ty)> {
    schema::STYLE_PROPS.iter().copied().filter(|(n, _)| *n != "states").collect()
}

/// Ombre complète : valeurs absentes à 0, couleur noire translucide.
fn shadow_with_defaults(mut shadow: Map<String, Json>) -> Json {
    for key in ["x", "y", "blur", "spread"] {
        shadow.entry(key).or_insert(json!(0));
    }
    shadow.entry("color").or_insert(json!("#00000040"));
    shadow.entry("inset").or_insert(json!(false));
    Json::Object(shadow)
}

/// `0%`, `37.5%`, `from` (0), `to` (100) → position en %.
fn keyframe_at(key: &str) -> Option<f64> {
    match key {
        "from" => Some(0.0),
        "to" => Some(100.0),
        _ => key.strip_suffix('%')?.trim().parse::<f64>().ok().filter(|at| (0.0..=100.0).contains(at)),
    }
}

impl Checker {
    /// `states:` : état → table de style (`hover: { background: … }`).
    pub(super) fn state_styles(&mut self, node: &Node, what: &str) -> Option<Json> {
        let mut out = Map::new();
        for (key, state, value) in self.entries(node, what)? {
            if !STATES.contains(&state) {
                self.fail::<()>(key, format!("{what} : état '{state}' inconnu (états : {}){}", STATES.join(", "), suggest(state, STATES)));
                continue;
            }
            if let Some(table) = self.table(value, &format!("states.{state}"), &state_props()) {
                out.insert(state.to_string(), Json::Object(table));
            }
        }
        Some(Json::Object(out))
    }

    /// `shadow` : une ombre (table) ou une liste d'ombres.
    pub(super) fn shadow(&mut self, node: &Node, what: &str) -> Option<Json> {
        let items: Vec<&Node> = match &node.kind {
            Kind::Seq(items) => items.iter().collect(),
            Kind::Map(_) => vec![node],
            _ => return self.fail(node, format!("{what} : ombre attendue ({{ x, y, blur, spread, color, inset }}) ou liste d'ombres")),
        };
        let shadows: Vec<Json> = items.into_iter()
            .filter_map(|item| self.table(item, what, SHADOW_PROPS))
            .map(shadow_with_defaults)
            .collect();
        Some(Json::Array(shadows))
    }

    /// `transition` : propriétés de style animées à chaque changement.
    pub(super) fn transition(&mut self, node: &Node, what: &str) -> Option<Json> {
        let table = self.table(node, what, TRANSITION_PROPS)?;
        let known: Vec<&str> = state_props().iter().map(|(n, _)| *n).filter(|n| *n != "transition").collect();
        for name in table.get("properties").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str) {
            if !known.contains(&name) {
                self.fail::<()>(node, format!("{what} : propriété '{name}' inconnue{}", suggest(name, &known)));
            }
        }
        Some(Json::Object(table))
    }

    /// `animation` d'un nœud : un nom déclaré sous `animations:`, ou
    /// `{ name, on }` ; normalisée en `{ name, on }`.
    pub(super) fn animation_ref(&mut self, node: &Node, what: &str) -> Option<Json> {
        let (name_node, trigger) = match &node.kind {
            Kind::Map(_) => {
                let table = self.table(node, what, &[("name", Ty::Text), ("on", Ty::Enum(TRIGGERS))])?;
                let Some(name) = table.get("name").and_then(Json::as_str).map(str::to_string) else {
                    return self.fail(node, format!("{what} : 'name' obligatoire"));
                };
                let on = table.get("on").and_then(Json::as_str).unwrap_or("appear").to_string();
                return self.known_animation(node, what, &name).map(|_| json!({ "name": name, "on": on }));
            }
            _ => (node, "appear"),
        };
        let name = text(name_node).unwrap_or_default().to_string();
        self.known_animation(node, what, &name).map(|_| json!({ "name": name, "on": trigger }))
    }

    fn known_animation(&mut self, node: &Node, what: &str, name: &str) -> Option<()> {
        if self.animations.contains(name) {
            return Some(());
        }
        let known: Vec<&str> = self.animations.iter().map(String::as_str).collect();
        self.fail(node, format!("{what} : animation '{name}' non déclarée sous 'animations:'{}", suggest(name, &known)))
    }

    /// Noms de `animations:`, connus avant les nœuds.
    pub(super) fn declare_animations(&mut self, node: &Node) {
        if let Kind::Map(pairs) = &node.kind {
            self.animations = pairs.iter().filter_map(|(k, _)| text(k)).filter(|n| is_ident(n)).map(str::to_string).collect::<BTreeSet<_>>();
        }
    }

    /// `animations:` : table nom → animation normalisée.
    pub(super) fn animations(&mut self, node: &Node) -> Map<String, Json> {
        let mut out = Map::new();
        for (key, name, value) in self.entries(node, "animations").unwrap_or_default() {
            if !is_ident(name) {
                self.fail::<()>(key, format!("nom d'animation '{name}' invalide (lettres, chiffres, '_')"));
                continue;
            }
            if let Some(animation) = self.animation(value, name) {
                out.insert(name.to_string(), animation);
            }
        }
        out
    }

    fn animation(&mut self, node: &Node, name: &str) -> Option<Json> {
        let what = format!("animations.{name}");
        let entries = self.entries(node, &what)?;
        let mut out = Map::new();
        let mut keyframes = None;
        let mut written = false;
        for &(key, prop, value) in &entries {
            match prop {
                "keyframes" => {
                    written = true;
                    keyframes = self.keyframes(value, &what);
                }
                "iterations" => match text(value) {
                    Some("infinite") => { out.insert("iterations".into(), json!("infinite")); }
                    _ => if let Some(n) = self.simple(value, Ty::Int(1, 1_000_000), &format!("'{what}.iterations'")) {
                        out.insert("iterations".into(), n);
                    },
                },
                _ => match ANIMATION_PROPS.iter().find(|(n, _)| *n == prop) {
                    Some(&(_, ty)) => if let Some(v) = self.simple(value, ty, &format!("'{what}.{prop}'")) {
                        out.insert(prop.to_string(), v);
                    },
                    None => {
                        let mut known: Vec<&str> = ANIMATION_PROPS.iter().map(|(n, _)| *n).collect();
                        known.extend(["iterations", "keyframes"]);
                        self.fail::<()>(key, format!("clé '{prop}' inconnue dans '{what}'{}", suggest(prop, &known)));
                    }
                },
            }
        }
        let Some(keyframes) = keyframes else {
            // Des `keyframes` invalides sont déjà signalés.
            return if written { None } else { self.fail(node, format!("'{what}' : 'keyframes' obligatoire")) };
        };
        out.entry("duration").or_insert(json!(300));
        out.entry("delay").or_insert(json!(0));
        out.entry("easing").or_insert(json!("ease"));
        out.entry("iterations").or_insert(json!(1));
        out.entry("direction").or_insert(json!("normal"));
        out.entry("fill").or_insert(json!("none"));
        out.insert("keyframes".into(), keyframes);
        Some(Json::Object(out))
    }

    /// `keyframes:` : étapes `N%` (ou `from`/`to`), triées, au moins deux.
    fn keyframes(&mut self, node: &Node, what: &str) -> Option<Json> {
        let mut steps: Vec<(f64, Map<String, Json>)> = Vec::new();
        for (key, at, value) in self.entries(node, &format!("{what}.keyframes"))? {
            let Some(position) = keyframe_at(at) else {
                self.fail::<()>(key, format!("'{what}.keyframes' : étape '{at}' invalide (de '0%' à '100%', 'from' ou 'to')"));
                continue;
            };
            if let Some(props) = self.table(value, &format!("{what}.keyframes.{at}"), KEYFRAME_PROPS) {
                steps.push((position, props));
            }
        }
        if steps.len() < 2 {
            return self.fail(node, format!("'{what}.keyframes' : au moins deux étapes attendues"));
        }
        steps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let steps: Vec<Json> = steps.into_iter().map(|(at, mut props)| {
            props.insert("at".into(), json!(at));
            Json::Object(props)
        }).collect();
        Some(Json::Array(steps))
    }
}
