//! Champs obligatoires (`required: true`, docs/oc-ui.md) : une action qui
//! soumet — `onSubmit` d'un champ, ou action d'un bouton `submit: true` —
//! n'est envoyée aux gestionnaires que si chaque champ obligatoire de son
//! formulaire est renseigné. Le formulaire est le conteneur `form: true` le
//! plus proche, sinon l'écran. Sinon, les champs vides passent à l'état
//! `invalid`, le premier reçoit le focus et l'action `onInvalid` du bouton
//! part avec leurs `id`. Vérifié ici, sur l'état gardé (saisies, `ui.set`,
//! liaisons), pour les deux moteurs à la fois.

use serde_json::{Map, Value};

use crate::{displayed, props, roots, with};

/// Chemin de nœuds de `node` jusqu'au nœud `id` (lui compris).
fn path_to<'t>(node: &'t Value, id: &str, path: &mut Vec<&'t Value>) -> bool {
    path.push(node);
    if node.get("id").and_then(Value::as_str) == Some(id) {
        return true;
    }
    for child in node.get("children").and_then(Value::as_array).into_iter().flatten() {
        if path_to(child, id, path) {
            return true;
        }
    }
    path.pop();
    false
}

/// Champs obligatoires de `node` et de ses descendants (`id`, type).
fn required_fields(node: &Value, out: &mut Vec<(String, String)>) {
    if node.get("required") == Some(&Value::Bool(true)) {
        if let (Some(id), Some(ty)) = (node.get("id").and_then(Value::as_str), node.get("type").and_then(Value::as_str)) {
            out.push((id.to_string(), ty.to_string()));
        }
    }
    for child in node.get("children").and_then(Value::as_array).into_iter().flatten() {
        required_fields(child, out);
    }
}

/// Valeur absente : `null`, texte vide ou blanc, case non cochée.
fn is_empty(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => true,
        Some(Value::String(text)) => text.trim().is_empty(),
        Some(_) => false,
    }
}

/// Propriété qui porte la valeur d'un champ de type `ty`.
fn value_prop(ty: &str) -> &'static str {
    if matches!(ty, "checkbox" | "switch") { "checked" } else { "value" }
}

/// Champs obligatoires (`id`, type) du formulaire qui contient `id`, ou de
/// `id` lui-même si c'est un conteneur (`whole`).
fn form_fields(this: i64, id: &str, whole: bool) -> Vec<(String, String)> {
    with(this, |ui| {
        let shown = displayed(&ui.tree, &ui.size_class);
        let mut fields = Vec::new();
        for root in roots(shown) {
            let mut path = Vec::new();
            if !path_to(root, id, &mut path) {
                continue;
            }
            let scope = match whole {
                true => *path.last().expect("nœud trouvé"),
                false => path.iter().rev().find(|n| n.get("form") == Some(&Value::Bool(true))).copied().unwrap_or(root),
            };
            required_fields(scope, &mut fields);
            break;
        }
        fields
    }).unwrap_or_default()
}

/// `id` des champs obligatoires vides parmi `fields`.
fn empty_fields(this: i64, fields: Vec<(String, String)>) -> Vec<String> {
    fields.into_iter()
        .filter(|(id, ty)| is_empty(props(this, id).get(value_prop(ty))))
        .map(|(id, _)| id)
        .collect()
}

/// L'action venue du nœud `node` soumet-elle un formulaire ?
pub(crate) fn submits(node: &Map<String, Value>, action: &str) -> bool {
    node.get("submit") == Some(&Value::Bool(true)) || node.get("onSubmit").and_then(Value::as_str) == Some(action)
}

/// Action qui soumet, venue du nœud `id` : `id` des champs obligatoires
/// vides, marqués `invalid` (le premier reçoit le focus). Vide : l'action
/// peut partir.
pub(crate) fn check_submit(this: i64, id: &str) -> Vec<String> {
    let empty = empty_fields(this, form_fields(this, id, false));
    for field in &empty {
        set_invalid(this, field, true);
    }
    if let (Some(first), Some(send)) = (empty.first(), crate::SEND_SET.get()) {
        send(this, first, "focus", &Value::Bool(true));
    }
    empty
}

/// État `invalid` d'un champ, gardé et envoyé au moteur s'il change.
pub(crate) fn set_invalid(this: i64, id: &str, invalid: bool) {
    let changed = with(this, |ui| {
        let state = ui.state.entry(id.to_string()).or_default();
        let before = state.get("invalid") == Some(&Value::Bool(true));
        state.insert("invalid".into(), Value::Bool(invalid));
        before != invalid
    }).unwrap_or(false);
    if let (true, Some(send)) = (changed, crate::SEND_SET.get()) {
        send(this, id, "invalid", &Value::Bool(invalid));
    }
}

/// `ui.validate(container)` : `id` des champs obligatoires vides du
/// conteneur `container` (sans marquer leur état).
pub fn validate(this: i64, container: &str) -> Vec<String> {
    crate::check_id(this, container);
    empty_fields(this, form_fields(this, container, true))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{is_empty, path_to, required_fields};

    #[test]
    fn empty_values() {
        assert!(is_empty(None) && is_empty(Some(&json!(null))) && is_empty(Some(&json!("  "))) && is_empty(Some(&json!(false))));
        assert!(!is_empty(Some(&json!("a"))) && !is_empty(Some(&json!(true))) && !is_empty(Some(&json!(0))));
    }

    #[test]
    fn the_nearest_form_holds_the_required_fields() {
        let layout = json!({ "type": "column", "children": [
            { "type": "input", "id": "search", "required": true },
            { "type": "card", "form": true, "children": [
                { "type": "input", "id": "name", "required": true },
                { "type": "checkbox", "id": "terms", "required": true },
                { "type": "button", "id": "save", "submit": true }
            ] }
        ] });
        let mut path = Vec::new();
        assert!(path_to(&layout, "save", &mut path));
        let form = path.iter().rev().find(|n| n.get("form") == Some(&json!(true))).expect("formulaire");
        let mut fields = Vec::new();
        required_fields(form, &mut fields);
        assert_eq!(fields, vec![("name".to_string(), "input".to_string()), ("terms".to_string(), "checkbox".to_string())]);
    }
}
