/// Tests unitaires — schéma des `.oc.ui` (src/core/ui_schema.d) : arbre
/// normalisé et erreurs situées à la ligne du fichier.

use std::path::Path;

use crate::core::ui_schema::{check_source, UiSpec};

/// Exemple de la fiche (docs/roadmap.d/langage-builtin-ui-multiplateforme.md §4).
const GARAGE: &str = r#"window:
  title: "Garage"
  width: 900
  height: 640
  theme: auto

styles:
  titre: { fontSize: 22, bold: true }

start: home
screens:
  home:
    layout:
      type: column
      spacing: 12
      padding: 16
      children:
        - { type: heading, value: "Mes voitures", style: titre }
        - type: row
          spacing: 8
          children:
            - { type: input, id: search, placeholder: "Rechercher", onSubmit: search, width: fill }
            - { type: iconButton, icon: search, label: "Rechercher", onClick: search }
        - type: list
          id: cars
          onSelect: open_car
          item:
            type: row
            spacing: 8
            children:
              - { type: text, bind: brand }
              - { type: text, bind: model }
  details:
    title: "Détail"
    layout:
      type: column
      padding: 16
      children:
        - { type: text, id: car_title, style: titre }
        - { type: button, text: "Retour", onClick: back }
"#;

fn ok(source: &str) -> UiSpec {
    match check_source(Path::new("t.oc.ui"), source) {
        Ok(spec) => spec,
        Err(errors) => panic!("erreurs inattendues : {:?}", errors.iter().map(|e| (e.line, &e.message)).collect::<Vec<_>>()),
    }
}

/// Erreurs (ligne, message) de `source`.
fn errors(source: &str) -> Vec<(usize, String)> {
    match check_source(Path::new("t.oc.ui"), source) {
        Ok(_) => panic!("erreur attendue"),
        Err(errors) => errors.into_iter().map(|e| (e.line, e.message)).collect(),
    }
}

/// Une seule erreur, à la ligne `line`, dont le message contient `part`.
fn single_error(source: &str, line: usize, part: &str) {
    let found = errors(source);
    assert_eq!(found.len(), 1, "une erreur attendue : {found:?}");
    assert_eq!(found[0].0, line, "ligne : {found:?}");
    assert!(found[0].1.contains(part), "« {part} » absent de : {}", found[0].1);
}

fn screen(children: &str) -> String {
    format!("layout:\n  type: column\n  children:\n{children}")
}

#[test]
fn garage_example_is_valid() {
    let spec = ok(GARAGE);
    assert_eq!(spec.tree["start"], "home");
    assert_eq!(spec.tree["window"]["width"], 900);
    assert_eq!(spec.tree["styles"]["titre"]["bold"], true);
    let ids: Vec<&str> = spec.ids.keys().map(String::as_str).collect();
    assert_eq!(ids, ["car_title", "cars", "search"]);
    assert_eq!(spec.ids["search"], "input");
    let actions: Vec<&str> = spec.actions.iter().map(String::as_str).collect();
    assert_eq!(actions, ["back", "open_car", "search"]);
    let row = &spec.tree["screens"]["home"]["layout"]["children"][1];
    assert_eq!(row["children"][0]["width"], "fill");
    assert_eq!(spec.tree["actions"][1], "open_car");
}

#[test]
fn root_layout_is_a_single_main_screen() {
    let spec = ok(&screen("    - { type: text, value: 42 }\n"));
    assert_eq!(spec.tree["start"], "main");
    assert_eq!(spec.tree["screens"]["main"]["layout"]["children"][0]["value"], "42");
}

#[test]
fn yaml_syntax_error_has_its_line() {
    let found = errors("window:\n  title: \"Garage\n  width: 900\n");
    assert_eq!(found.len(), 1);
    assert!(found[0].1.starts_with("YAML invalide"), "{}", found[0].1);
}

#[test]
fn unknown_node_type_suggests() {
    single_error(&screen("    - type: buton\n"), 4, "'buton' inconnu (vouliez-vous dire 'button' ?)");
}

#[test]
fn unknown_property_is_located() {
    single_error(&screen("    - type: button\n      text: \"Ok\"\n      onclik: save\n"), 6, "propriété 'onclik' inconnue pour 'button' (vouliez-vous dire 'onClick' ?)");
}

#[test]
fn node_without_type() {
    single_error(&screen("    - { text: \"Ok\" }\n"), 4, "nœud sans 'type'");
}

#[test]
fn duplicate_id_names_first_line() {
    single_error(&screen("    - { type: input, id: name }\n    - { type: text, id: name }\n"), 5, "id 'name' déjà utilisé ligne 4");
}

#[test]
fn invalid_enum_value() {
    single_error(&screen("    - { type: input, kind: pasword }\n"), 4, "valeur 'pasword' inconnue");
}

#[test]
fn unquoted_color_is_explained() {
    single_error(&screen("    - type: text\n      color: #ff0000\n"), 5, "'#' commence un commentaire YAML");
}

#[test]
fn unquoted_color_in_flow_map_hints() {
    let found = errors(&screen("    - { type: text, color: #ff0000 }\n    - { type: spacer }\n"));
    assert_eq!(found.len(), 1);
    assert!(found[0].1.contains("couleur entre guillemets"), "{}", found[0].1);
}

#[test]
fn undeclared_style() {
    single_error(&screen("    - { type: text, style: titre }\n"), 4, "style 'titre' non déclaré");
}

#[test]
fn bind_outside_list_item() {
    single_error(&screen("    - { type: text, bind: brand }\n"), 4, "gabarit 'item'");
}

#[test]
fn required_properties() {
    single_error(&screen("    - { type: iconButton, icon: add }\n"), 4, "'iconButton' : propriété 'label' obligatoire");
    single_error(&screen("    - { type: image, label: \"Logo\" }\n"), 4, "propriété 'src' ou 'bind' obligatoire");
}

#[test]
fn invalid_action_name() {
    single_error(&screen("    - { type: button, text: Ok, onClick: \"save form\" }\n"), 4, "nom d'action attendu");
}

#[test]
fn out_of_range_and_wrong_types() {
    single_error(&screen("    - { type: heading, value: T, level: 4 }\n"), 4, "4 hors de l'intervalle [1, 3]");
    single_error(&screen("    - { type: checkbox, checked: oui }\n"), 4, "booléen attendu");
    single_error(&screen("    - { type: text, width: large }\n"), 4, "'fill' ou 'wrap'");
}

#[test]
fn platform_overrides() {
    let spec = ok(&screen("    - { type: button, text: Ok, android: { variant: text } }\n"));
    assert_eq!(spec.tree["screens"]["main"]["layout"]["children"][0]["android"]["variant"], "text");
    single_error(&screen("    - { type: button, text: Ok, tauri: { id: x } }\n"), 4, "'id' ne se surcharge pas");
    single_error(&screen("    - { type: button, text: Ok, tauri: { colour: \"#fff\" } }\n"), 4, "inconnue pour 'button' (dans 'tauri')");
}

#[test]
fn start_must_name_a_screen() {
    let source = GARAGE.replace("start: home", "start: hom");
    single_error(&source, 10, "écran 'hom' non déclaré");
    let source = GARAGE.replace("start: home\n", "");
    single_error(&source, 1, "'start' doit nommer");
}

#[test]
fn unknown_root_and_window_keys() {
    single_error(&format!("windw: {{ title: T }}\n{}", screen("    - { type: spacer }\n")), 1, "vouliez-vous dire 'window' ?");
    single_error(&format!("window: {{ titel: T }}\n{}", screen("    - { type: spacer }\n")), 1, "clé 'titel' inconnue dans 'window'");
}

#[test]
fn window_platform_options() {
    let spec = ok(&format!("window:\n  tauri: {{ alwaysOnTop: true }}\n  android: {{ orientation: portrait }}\n{}", screen("    - { type: spacer }\n")));
    assert_eq!(spec.tree["window"]["tauri"]["alwaysOnTop"], true);
    single_error(&format!("window:\n  android: {{ orientation: sideways }}\n{}", screen("    - { type: spacer }\n")), 2, "'sideways' inconnue");
}

#[test]
fn duplicate_key() {
    single_error(&screen("    - type: text\n      value: a\n      value: b\n"), 6, "clé 'value' en double");
}

#[test]
fn webview_single_source() {
    single_error(&screen("    - { type: webview, url: \"http://x\", html: \"<p>\" }\n"), 4, "une seule source");
}

#[test]
fn options_are_normalized() {
    let spec = ok(&screen("    - { type: select, options: [a, { value: b, text: Bé }] }\n"));
    let options = &spec.tree["screens"]["main"]["layout"]["children"][0]["options"];
    assert_eq!(options[0]["text"], "a");
    assert_eq!(options[1]["text"], "Bé");
}

#[test]
fn menu_entries() {
    let spec = ok(&format!("menu:\n  - text: Fichier\n    items:\n      - {{ text: Quitter, action: quit }}\n{}", screen("    - { type: spacer }\n")));
    assert!(spec.actions.contains("quit"));
    single_error(&format!("menu:\n  - {{ action: quit }}\n{}", screen("    - { type: spacer }\n")), 2, "sans 'text'");
}

#[test]
fn several_errors_are_sorted() {
    let found = errors(&screen("    - { type: text, colr: \"#fff\" }\n    - { type: nope }\n"));
    let lines: Vec<usize> = found.iter().map(|(l, _)| *l).collect();
    assert_eq!(lines, [4, 5]);
}

#[test]
fn aliases_and_empty_file_are_refused() {
    single_error("", 1, "vide");
    single_error("a: &x 1\nb: *x\n", 2, "alias");
}
