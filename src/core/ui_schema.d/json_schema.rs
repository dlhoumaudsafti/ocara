// Schéma JSON (draft-07) des `.oc.ui`, produit depuis les tables du
// compilateur (`ocara --ui-schema`) : complétion et validation dans les
// éditeurs (extension VS Code, `yamlValidation`). La validation qui fait foi
// reste celle du compilateur (références de styles, `id` uniques, `bind`…).
//
// Le `layout` d'un composant (`components:`) a sa propre définition,
// `componentNode` : toute propriété y accepte aussi un paramètre `$nom`,
// remplacé à la compilation (`background: $tone`). `node` reste la forme
// stricte, lue aussi par le designer de l'extension (catalog.ts).

use serde_json::{json, Map, Value as Json};

use super::icons;
use super::schema::{self, Ty};

/// Paramètre d'un composant (`$nom`), seul dans la valeur.
const PARAM: &str = "^\\$[A-Za-z_][A-Za-z0-9_]*$";

/// Définition d'un nœud : stricte (`node`) ou dans un composant (`componentNode`).
#[derive(Clone, Copy)]
enum Within {
    Screen,
    Component,
}

impl Within {
    fn node_ref(self) -> Json {
        match self {
            Within::Screen => json!({ "$ref": "#/definitions/node" }),
            Within::Component => json!({ "$ref": "#/definitions/componentNode" }),
        }
    }
}

/// Valeurs des propriétés de `componentNode` (`definitions.values`) :
/// chacune écrite une fois, référencée par chaque propriété qui l'emploie.
#[derive(Default)]
struct Values(Vec<Json>);

impl Values {
    /// `$ref` vers `value`, rangée à sa première rencontre.
    fn reference(&mut self, value: Json) -> Json {
        let index = self.0.iter().position(|v| *v == value).unwrap_or_else(|| {
            self.0.push(value);
            self.0.len() - 1
        });
        json!({ "$ref": format!("#/definitions/values/v{index}") })
    }

    fn definitions(self) -> Json {
        let mut out: Map<String, Json> = self.0.into_iter().enumerate().map(|(i, v)| (format!("v{i}"), v)).collect();
        out.insert("param".into(), json!({ "type": "string", "pattern": PARAM }));
        Json::Object(out)
    }
}

fn ty(t: Ty) -> Json {
    ty_within(t, Within::Screen)
}

/// Type `t` ; dans un composant, une propriété accepte aussi `$nom`.
fn prop_ty(t: Ty, within: Within, values: &mut Values) -> Json {
    let strict = ty_within(t, within);
    match (within, t) {
        (Within::Screen, _) | (_, Ty::Node | Ty::Children | Ty::Override | Ty::Binds) => strict,
        (Within::Component, _) => values.reference(json!({ "anyOf": [strict, { "$ref": "#/definitions/values/param" }] })),
    }
}

fn ty_within(t: Ty, within: Within) -> Json {
    match t {
        Ty::Text => json!({ "type": ["string", "number", "boolean"] }),
        Ty::Bool => json!({ "type": "boolean" }),
        Ty::Int(min, max) => json!({ "type": "integer", "minimum": min, "maximum": max }),
        Ty::Number(min, max) => {
            let mut n = json!({ "type": "number" });
            if min > f64::MIN { n["minimum"] = json!(min); }
            if max < f64::MAX { n["maximum"] = json!(max); }
            n
        }
        Ty::Length => json!({ "anyOf": [{ "type": "number", "minimum": 0 }, { "enum": ["fill", "wrap"] }] }),
        Ty::Spacing => json!({ "anyOf": [
            { "type": "number", "minimum": 0 },
            { "type": "array", "items": { "type": "number", "minimum": 0 }, "minItems": 2, "maxItems": 4 },
        ] }),
        Ty::Color => json!({ "type": "string", "pattern": "^(#([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})|rgba?\\(.*\\))$" }),
        Ty::Enum(values) => json!({ "enum": values }),
        // `a.b` : `id` interne d'un composant posé avec l'`id` `a` (nodes.rs).
        Ty::Id => json!({ "type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]*(\\.[A-Za-z_][A-Za-z0-9_]*)*$" }),
        // `self.nom` : action propre d'un composant (values.rs, un point au plus).
        Ty::Action => json!({ "type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]*(\\.[A-Za-z_][A-Za-z0-9_]*)?$" }),
        Ty::StyleRef | Ty::Field => json!({ "type": "string" }),
        Ty::Icon => json!({ "enum": icons::names() }),
        Ty::Node => within.node_ref(),
        Ty::Children => json!({ "type": "array", "items": within.node_ref() }),
        Ty::Strings => json!({ "type": "array", "items": { "type": "string" } }),
        Ty::Options => json!({ "type": "array", "items": { "anyOf": [
            { "type": ["string", "number"] },
            { "type": "object", "properties": { "value": {}, "text": { "type": "string" } }, "required": ["value"] },
        ] } }),
        Ty::Menu => json!({ "type": "array", "items": { "$ref": "#/definitions/menuItem" } }),
        Ty::ActionMap => json!({ "type": "object", "additionalProperties": ty(Ty::Action) }),
        Ty::Override => json!({ "type": "object" }),
        Ty::Binds => json!({ "type": "object", "additionalProperties": { "type": "string" } }),
    }
}

fn object(props: &[(&str, Ty)]) -> Json {
    object_within(props, Within::Screen, &mut Values::default())
}

fn object_within(props: &[(&str, Ty)], within: Within, values: &mut Values) -> Json {
    let properties: Map<String, Json> = props.iter().map(|(name, t)| (name.to_string(), prop_ty(*t, within, values))).collect();
    json!({ "type": "object", "properties": properties, "additionalProperties": false })
}

/// Propriétés d'un nœud de type `name` ; `tauri`/`android` reprennent les
/// mêmes, sans la structure.
fn node_case(name: &str, within: Within, values: &mut Values) -> Json {
    let all: Vec<(&str, Ty)> = schema::node_props(name).unwrap_or_default().iter()
        .chain(schema::COMMON_PROPS)
        .chain(schema::STYLE_PROPS)
        .copied()
        .collect();
    let overridable: Vec<(&str, Ty)> = all.iter().copied()
        .filter(|(n, _)| !matches!(*n, "type" | "id" | "children" | "item" | "tauri" | "android"))
        .collect();
    let mut properties: Map<String, Json> = all.iter().map(|(n, t)| (n.to_string(), prop_ty(*t, within, values))).collect();
    properties.insert("type".into(), json!({ "const": name }));
    properties.insert("tauri".into(), object_within(&overridable, within, values));
    properties.insert("android".into(), object_within(&overridable, within, values));
    let required: Vec<&str> = schema::required_props(name).iter()
        .filter(|r| !r.contains('|'))
        .copied()
        .collect();
    json!({
        "if": { "properties": { "type": { "const": name } }, "required": ["type"] },
        "then": { "properties": properties, "required": required, "additionalProperties": false },
    })
}

pub fn schema() -> Json {
    let param_types: Vec<&str> = super::components::PARAM_TYPES.iter().map(|(n, _)| *n).collect();
    let mut window = object(schema::WINDOW_PROPS);
    window["properties"]["tauri"] = object(schema::WINDOW_TAURI);
    window["properties"]["android"] = object(schema::WINDOW_ANDROID);
    let mut values = Values::default();
    let mut node = |within: Within| json!({
        "type": "object",
        "required": ["type"],
        // Type prédéfini, ou nom d'un composant déclaré sous `components:`.
        "properties": { "type": { "anyOf": [{ "enum": schema::NODE_TYPES }, { "type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]*$" }] } },
        "allOf": schema::NODE_TYPES.iter().map(|t| node_case(t, within, &mut values)).collect::<Vec<_>>(),
    });
    let (screen_node, component_node) = (node(Within::Screen), node(Within::Component));
    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "Interface ocara.UI (.oc.ui)",
        "description": "Généré par `ocara --ui-schema` depuis src/core/ui_schema.d (docs/oc-ui.md).",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "window": window,
            "styles": {
                "type": "object",
                "properties": {
                    "dark": { "type": "object", "additionalProperties": object(schema::STYLE_PROPS) },
                    "light": { "type": "object", "additionalProperties": object(schema::STYLE_PROPS) },
                },
                "additionalProperties": object(schema::STYLE_PROPS),
            },
            "components": {
                "type": "object",
                "properties": { "includes": { "type": "array", "items": { "type": "string", "pattern": "\\.oc\\.ui$" } } },
                "additionalProperties": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["layout"],
                    "properties": {
                        "params": {
                            "type": "object",
                            "additionalProperties": {
                                "anyOf": [
                                    { "enum": param_types },
                                    {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "required": ["type"],
                                        "properties": { "type": { "enum": param_types }, "required": { "type": "boolean" }, "default": {} },
                                    },
                                ],
                            },
                        },
                        "layout": { "$ref": "#/definitions/componentNode" },
                    },
                },
            },
            "start": { "type": "string" },
            "screens": {
                "type": "object",
                "properties": { "includes": { "type": "array", "items": { "type": "string", "pattern": "\\.oc\\.ui$" } } },
                "additionalProperties": object(schema::SCREEN_PROPS),
            },
            "layout": { "$ref": "#/definitions/node" },
            "menu": ty(Ty::Menu),
        },
        "definitions": {
            "node": screen_node,
            "componentNode": component_node,
            "values": values.definitions(),
            "menuItem": object(schema::MENU_PROPS),
        },
    })
}
