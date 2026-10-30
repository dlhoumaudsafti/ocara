// Schéma JSON (draft-07) des `.oc.ui`, produit depuis les tables du
// compilateur (`ocara --ui-schema`) : complétion et validation dans les
// éditeurs (extension VS Code, `yamlValidation`). La validation qui fait foi
// reste celle du compilateur (références de styles, `id` uniques, `bind`…).

use serde_json::{json, Map, Value as Json};

use super::icons;
use super::schema::{self, Ty};

fn ty(t: Ty) -> Json {
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
        Ty::Color => json!({ "type": "string", "pattern": "^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$" }),
        Ty::Enum(values) => json!({ "enum": values }),
        Ty::Id | Ty::Action => json!({ "type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]*$" }),
        Ty::StyleRef | Ty::Field => json!({ "type": "string" }),
        Ty::Icon => json!({ "enum": icons::names() }),
        Ty::Node => json!({ "$ref": "#/definitions/node" }),
        Ty::Children => json!({ "type": "array", "items": { "$ref": "#/definitions/node" } }),
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
    let properties: Map<String, Json> = props.iter().map(|(name, t)| (name.to_string(), ty(*t))).collect();
    json!({ "type": "object", "properties": properties, "additionalProperties": false })
}

/// Propriétés d'un nœud de type `name` ; `tauri`/`android` reprennent les
/// mêmes, sans la structure.
fn node_case(name: &str) -> Json {
    let all: Vec<(&str, Ty)> = schema::node_props(name).unwrap_or_default().iter()
        .chain(schema::COMMON_PROPS)
        .chain(schema::STYLE_PROPS)
        .copied()
        .collect();
    let overridable: Vec<(&str, Ty)> = all.iter().copied()
        .filter(|(n, _)| !matches!(*n, "type" | "id" | "children" | "item" | "tauri" | "android"))
        .collect();
    let mut properties: Map<String, Json> = all.iter().map(|(n, t)| (n.to_string(), ty(*t))).collect();
    properties.insert("type".into(), json!({ "const": name }));
    properties.insert("tauri".into(), object(&overridable));
    properties.insert("android".into(), object(&overridable));
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
    let mut window = object(schema::WINDOW_PROPS);
    window["properties"]["tauri"] = object(schema::WINDOW_TAURI);
    window["properties"]["android"] = object(schema::WINDOW_ANDROID);
    let node = json!({
        "type": "object",
        "required": ["type"],
        "properties": { "type": { "enum": schema::NODE_TYPES } },
        "allOf": schema::NODE_TYPES.iter().map(|t| node_case(t)).collect::<Vec<_>>(),
    });
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
            "start": { "type": "string" },
            "screens": {
                "type": "object",
                "properties": { "includes": { "type": "array", "items": { "type": "string", "pattern": "\\.oc\\.ui$" } } },
                "additionalProperties": object(schema::SCREEN_PROPS),
            },
            "layout": { "$ref": "#/definitions/node" },
            "menu": ty(Ty::Menu),
        },
        "definitions": { "node": node, "menuItem": object(schema::MENU_PROPS) },
    })
}
