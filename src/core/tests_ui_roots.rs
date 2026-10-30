/// Tests unitaires — liaisons déclaratives des `.oc.ui`
/// (src/core/ui_schema.d/roots.rs) : `bind`/`binds` hors d'une liste rangés
/// dans `roots`, `id` donné aux nœuds liés, variantes de taille réunies.

use std::path::Path;

use serde_json::json;

use crate::core::ui_schema::{check_interface, check_source};

fn tree(children: &str) -> serde_json::Value {
    let source = format!("layout:\n  type: column\n  children:\n{children}");
    match check_source(Path::new("t.oc.ui"), &source) {
        Ok(spec) => spec.tree,
        Err(errors) => panic!("erreurs inattendues : {:?}", errors.iter().map(|e| &e.message).collect::<Vec<_>>()),
    }
}

fn first_error(children: &str) -> String {
    let source = format!("layout:\n  type: column\n  children:\n{children}");
    check_source(Path::new("t.oc.ui"), &source).expect_err("erreur attendue")[0].message.clone()
}

#[test]
fn bindings_move_from_the_nodes_to_the_roots() {
    let tree = tree(concat!(
        "    - { type: input, id: name, bind: user.name }\n",
        "    - { type: checkbox, id: news, bind: user.newsletter }\n",
        "    - { type: slider, id: volume, binds: { value: user.volume, visible: user.active } }\n",
    ));
    let children = &tree["screens"]["main"]["layout"]["children"];
    assert!(children[0].get("bind").is_none() && children[2].get("binds").is_none());
    assert_eq!(tree["roots"]["user"]["paths"], json!(["name", "newsletter", "volume", "active"]));
    assert_eq!(tree["roots"]["user"]["nodes"], json!([
        { "id": "name", "prop": "value", "path": 0, "kind": "text" },
        { "id": "news", "prop": "checked", "path": 1, "kind": "bool" },
        { "id": "volume", "prop": "value", "path": 2, "kind": "number" },
        { "id": "volume", "prop": "visible", "path": 3, "kind": "bool" },
    ]));
}

#[test]
fn bound_nodes_without_id_get_one() {
    let tree = tree("    - { type: text, bind: user.name }\n    - { type: heading, bind: user.name }\n    - { type: text, bind: car.address.city }\n");
    let children = &tree["screens"]["main"]["layout"]["children"];
    assert_eq!(children[0]["id"], "@user.name");
    assert_eq!(children[1]["id"], "@user.name#2");
    assert_eq!(children[2]["id"], "@car.address.city");
    assert_eq!(tree["roots"]["user"]["paths"], json!(["name"]));
    assert_eq!(tree["ids"]["@user.name#2"], "heading");
}

#[test]
fn a_list_template_keeps_its_element_fields() {
    let tree = tree("    - { type: list, id: cars, item: { type: text, bind: brand.name } }\n");
    assert_eq!(tree["screens"]["main"]["layout"]["children"][0]["item"]["bind"], "brand.name");
    assert!(tree.get("roots").is_none());
}

#[test]
fn bind_outside_a_list_needs_a_root_and_a_bindable_property() {
    assert!(first_error("    - { type: text, bind: name }\n").contains("'racine.champ' attendu"));
    assert!(first_error("    - { type: text, binds: { value: name } }\n").contains("'racine.champ' attendu"));
    assert!(first_error("    - { type: button, text: Ok, binds: { onClick: user.action } }\n").contains("ne peut pas être liée"));
}

#[test]
fn size_variants_add_their_bindings_to_the_base_roots() {
    let dir = std::env::temp_dir().join(format!("ocara_ui_roots_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("x.oc.ui"), "layout:\n  type: column\n  children:\n    - { type: input, id: name, bind: user.name }\n").unwrap();
    std::fs::write(dir.join("x.lg.oc.ui"), "layout:\n  type: row\n  children:\n    - { type: input, id: name, bind: user.name }\n    - { type: text, id: city, bind: user.city }\n").unwrap();
    let spec = check_interface(&dir.join("x.oc.ui"));
    std::fs::remove_dir_all(&dir).ok();
    let roots = &spec.expect("interface valide").tree["roots"]["user"];
    assert_eq!(roots["paths"], json!(["name", "city"]));
    assert_eq!(roots["nodes"], json!([
        { "id": "name", "prop": "value", "path": 0, "kind": "text" },
        { "id": "city", "prop": "value", "path": 1, "kind": "text" },
    ]));
}

#[test]
fn form_fields_and_buttons_get_an_id() {
    let tree = tree("    - { type: input, required: true }\n    - { type: input, onSubmit: go }\n    - { type: button, text: Ok, submit: true, onClick: save }\n    - { type: input }\n");
    let children = &tree["screens"]["main"]["layout"]["children"];
    assert_eq!(children[0]["id"], "@input");
    assert_eq!(children[1]["id"], "@input#2");
    assert_eq!(children[2]["id"], "@button");
    assert!(children[3].get("id").is_none());
}

#[test]
fn events_on_any_node_and_on_invalid_needs_submit() {
    let tree = tree("    - { type: text, value: a, onHover: h, onLeave: l, onFocus: f, onBlur: b, onLongPress: p, onDoubleClick: d, onSwipe: s }\n");
    assert_eq!(tree["actions"], json!(["b", "d", "f", "h", "l", "p", "s"]));
    assert!(first_error("    - { type: button, text: Ok, onClick: save, onInvalid: missing }\n").contains("'onInvalid' n'a de sens qu'avec 'submit: true'"));
}
