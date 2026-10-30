/// Tests unitaires — styles d'état, ombres, transitions, animations et lien
/// des `.oc.ui` (src/core/ui_schema.d/effects.rs).

use std::path::Path;

use serde_json::json;

use crate::core::ui_schema::check_source;

fn tree(source: &str) -> serde_json::Value {
    match check_source(Path::new("t.oc.ui"), source) {
        Ok(spec) => spec.tree,
        Err(errors) => panic!("erreurs inattendues : {:?}", errors.iter().map(|e| &e.message).collect::<Vec<_>>()),
    }
}

fn first_error(source: &str) -> String {
    check_source(Path::new("t.oc.ui"), source).expect_err("erreur attendue")[0].message.clone()
}

const PULSE: &str = "animations:\n  pulse:\n    duration: 800\n    iterations: infinite\n    keyframes:\n      from: { opacity: 1 }\n      50%: { scale: 1.1 }\n      to: { opacity: 0.5 }\n";

#[test]
fn state_tables_shadow_and_transition_in_a_style() {
    let tree = tree(concat!(
        "styles:\n  b:\n    states: { hover: { background: \"#fff\" } }\n    shadow: { y: 2, blur: 6 }\n    transition: { duration: 150, properties: [background] }\n",
        "layout: { type: checkbox, checked: true, style: b, states: { focus: { bold: true }, checked: { color: \"#000\" } } }\n",
    ));
    assert_eq!(tree["styles"]["b"]["states"]["hover"], json!({ "background": "#ffffff" }));
    assert_eq!(tree["styles"]["b"]["shadow"], json!([{ "x": 0, "y": 2, "blur": 6, "spread": 0, "color": "#00000040", "inset": false }]));
    assert_eq!(tree["styles"]["b"]["transition"], json!({ "duration": 150, "properties": ["background"] }));
    let layout = &tree["screens"]["main"]["layout"];
    assert_eq!(layout["checked"], true);
    assert_eq!(layout["states"], json!({ "focus": { "bold": true }, "checked": { "color": "#000000" } }));
}

#[test]
fn state_and_transition_errors() {
    assert!(first_error("layout: { type: text, value: a, states: { hover: { onClick: x } } }\n").contains("clé 'onClick' inconnue"));
    assert!(first_error("layout: { type: text, value: a, states: { hover: { states: {} } } }\n").contains("clé 'states' inconnue"));
    assert!(first_error("layout: { type: text, value: a, states: { hovr: { bold: true } } }\n").contains("état 'hovr' inconnu"));
    assert!(first_error("layout: { type: text, value: a, transition: { properties: [colour] } }\n").contains("propriété 'colour' inconnue"));
}

#[test]
fn animations_are_normalized_and_referenced() {
    let tree = tree(&format!("{PULSE}layout: {{ type: card, animation: pulse, children: [{{ type: text, value: a, animation: {{ name: pulse, on: hover }} }}] }}\n"));
    let pulse = &tree["animations"]["pulse"];
    assert_eq!(pulse["iterations"], "infinite");
    assert_eq!(pulse["easing"], "ease");
    assert_eq!(pulse["keyframes"], json!([{ "at": 0.0, "opacity": 1 }, { "at": 50.0, "scale": 1.1 }, { "at": 100.0, "opacity": 0.5 }]));
    let layout = &tree["screens"]["main"]["layout"];
    assert_eq!(layout["animation"], json!({ "name": "pulse", "on": "appear" }));
    assert_eq!(layout["children"][0]["animation"], json!({ "name": "pulse", "on": "hover" }));
}

#[test]
fn animation_errors() {
    assert!(first_error("layout: { type: text, value: a, animation: puls }\n").contains("animation 'puls' non déclarée"));
    assert!(first_error("animations:\n  a:\n    keyframes:\n      0%: { opacity: 1 }\nlayout: { type: text, value: a }\n").contains("au moins deux étapes"));
    assert!(first_error("animations:\n  a:\n    keyframes:\n      0%: { padding: 1 }\n      100%: { opacity: 1 }\nlayout: { type: text, value: a }\n").contains("clé 'padding' inconnue"));
    assert!(first_error("animations:\n  a:\n    keyframes:\n      120%: { opacity: 1 }\n      100%: { opacity: 0 }\nlayout: { type: text, value: a }\n").contains("étape '120%' invalide"));
    assert!(first_error("animations:\n  a: { duration: 10 }\nlayout: { type: text, value: a }\n").contains("'keyframes' obligatoire"));
}

#[test]
fn a_link_needs_a_text_and_gets_an_id() {
    let tree = tree("layout: { type: link, text: Aide, onClick: help }\n");
    assert_eq!(tree["screens"]["main"]["layout"]["id"], "@link");
    assert!(first_error("layout: { type: link, onClick: help }\n").contains("'text' obligatoire"));
}
