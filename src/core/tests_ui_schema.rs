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
    single_error(&screen("    - { type: webview, id: w, url: \"http://x\", html: \"<p>\" }\n"), 4, "une seule source");
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

#[test]
fn icons_are_checked_and_embedded() {
    let spec = ok(&screen("    - { type: iconButton, icon: search, label: Chercher }\n    - { type: button, text: Ok, icon: check }\n"));
    let icons = spec.tree["icons"].as_object().unwrap();
    let names: Vec<&str> = icons.keys().map(String::as_str).collect();
    assert_eq!(names, ["check", "search"]);
    assert!(icons["search"].as_str().unwrap().starts_with("M15.5"));
    single_error(&screen("    - { type: icon, name: serch }\n"), 4, "icône 'serch' inconnue");
}

#[test]
fn components_example_is_valid() {
    let spec = ok(include_str!("../../examples/ui/components.oc.ui"));
    assert_eq!(spec.tree["start"], "form");
    assert_eq!(spec.ids["people"], "list");
}

#[test]
fn local_images_are_embedded() {
    let dir = std::env::temp_dir().join(format!("ocara_ui_images_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("logo.png"), b"\x89PNG").unwrap();
    let file = dir.join("t.oc.ui");
    let source = screen("    - { type: image, src: logo.png, label: Logo }\n    - { type: image, src: \"https://x.org/a.png\", label: A }\n");
    let spec = check_source(&file, &source).expect("arbre attendu");
    let children = &spec.tree["screens"]["main"]["layout"]["children"];
    assert_eq!(children[0]["src"], "data:image/png;base64,iVBORw==");
    assert_eq!(children[1]["src"], "https://x.org/a.png");
    let errors = check_source(&file, &screen("    - { type: image, src: absent.png, label: A }\n")).unwrap_err();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(errors[0].line, 4);
    assert!(errors[0].message.contains("absent.png"), "{}", errors[0].message);
}

/// Le schéma livré avec l'extension VS Code suit les tables du compilateur
/// (`ocara --ui-schema > tools/highlight/vsode/schemas/oc-ui.schema.json`).
#[test]
fn editor_schema_is_up_to_date() {
    let shipped: serde_json::Value = serde_json::from_str(include_str!("../../tools/highlight/vsode/schemas/oc-ui.schema.json")).unwrap();
    assert!(shipped == crate::core::ui_schema::json_schema::schema(),
        "schéma périmé : ocara --ui-schema > tools/highlight/vsode/schemas/oc-ui.schema.json");
}

/// Dossier temporaire avec les fichiers `(nom, contenu)` ; rend son chemin.
fn files(name: &str, entries: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ocara_ui_{}_{}", name, std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    for (file, content) in entries {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    dir
}

const ONE_SCREEN: &str = "screens:\n  more:\n    layout: { type: column, children: [{ type: text, id: more_text, value: x }] }\n";

#[test]
fn includes_add_screens_and_locate_errors() {
    use crate::core::ui_schema::check_file;
    let main = "start: home\nscreens:\n  home:\n    layout: { type: column, children: [] }\n  includes:\n    - ui.d/more.oc.ui\n";
    let dir = files("inc_ok", &[("main.oc.ui", main), ("ui.d/more.oc.ui", ONE_SCREEN)]);
    let spec = check_file(&dir.join("main.oc.ui")).expect("arbre attendu");
    assert!(spec.tree["screens"]["more"].is_object());
    assert_eq!(spec.ids["more_text"], "text");
    assert_eq!(spec.files.len(), 2);

    let bad = "screens:\n  more:\n    layout: { type: column, children: [{ type: buton }] }\n";
    let dir = files("inc_err", &[("main.oc.ui", main), ("ui.d/more.oc.ui", bad)]);
    let errors = check_file(&dir.join("main.oc.ui")).unwrap_err();
    assert!(errors[0].file.ends_with("ui.d/more.oc.ui"), "{:?}", errors[0].file);
    assert_eq!(errors[0].line, 3);

    let twice = "start: home\nscreens:\n  home:\n    layout: { type: column, children: [] }\n  includes:\n    - home.oc.ui\n";
    let again = "screens:\n  home:\n    layout: { type: column, children: [] }\n";
    let dir = files("inc_dup", &[("main.oc.ui", twice), ("home.oc.ui", again)]);
    let errors = check_file(&dir.join("main.oc.ui")).unwrap_err();
    assert!(errors[0].message.contains("écran 'home' déjà déclaré"), "{}", errors[0].message);

    let cycle = "screens:\n  more:\n    layout: { type: column, children: [] }\n  includes:\n    - main.oc.ui\n";
    let dir = files("inc_cycle", &[("main.oc.ui", main), ("ui.d/more.oc.ui", cycle)]);
    let errors = check_file(&dir.join("main.oc.ui")).unwrap_err();
    std::fs::remove_dir_all(&dir).ok();
    assert!(errors.iter().any(|e| e.message.contains("lecture de") || e.message.contains("déjà inclus")), "{:?}", errors);
}

#[test]
fn included_files_declare_their_styles() {
    use crate::core::ui_schema::check_file;
    // `a` utilise un style de `b`, inclus après lui : les noms sont connus d'avance.
    let main = "styles:\n  base: { bold: true }\nstart: a\nscreens:\n  includes:\n    - ui.d/a.oc.ui\n    - ui.d/b.oc.ui\n";
    let a = "styles:\n  a_title: { fontSize: 22 }\nscreens:\n  a:\n    layout: { type: column, children: [{ type: text, style: b_note }, { type: text, style: base }] }\n";
    let b = "styles:\n  b_note: { italic: true }\nscreens:\n  b:\n    layout: { type: column, children: [{ type: text, style: a_title }] }\n";
    let dir = files("inc_styles", &[("main.oc.ui", main), ("ui.d/a.oc.ui", a), ("ui.d/b.oc.ui", b)]);
    let spec = check_file(&dir.join("main.oc.ui")).expect("arbre attendu");
    for name in ["base", "a_title", "b_note"] {
        assert!(spec.tree["styles"][name].is_object(), "style {name} absent : {}", spec.tree["styles"]);
    }
    assert_eq!(spec.tree["styles"]["a_title"]["fontSize"], 22.0);

    let dup = "styles:\n  base: { italic: true }\nscreens:\n  b:\n    layout: { type: column, children: [] }\n";
    let dir = files("inc_styles_dup", &[("main.oc.ui", main), ("ui.d/a.oc.ui", a), ("ui.d/b.oc.ui", dup)]);
    let errors = check_file(&dir.join("main.oc.ui")).unwrap_err();
    assert!(errors.iter().any(|e| e.file.ends_with("ui.d/b.oc.ui") && e.line == 2 && e.message.contains("style 'base' déjà déclaré")), "{:?}", errors);

    let unknown = "window: { title: x }\nscreens:\n  b:\n    layout: { type: column, children: [] }\n";
    let dir = files("inc_styles_key", &[("main.oc.ui", main), ("ui.d/a.oc.ui", a), ("ui.d/b.oc.ui", unknown)]);
    let errors = check_file(&dir.join("main.oc.ui")).unwrap_err();
    std::fs::remove_dir_all(&dir).ok();
    assert!(errors.iter().any(|e| e.message.contains("seules 'screens', 'styles' et 'components'")), "{:?}", errors);
}

#[test]
fn list_grid_options() {
    let grid = "    - { type: list, id: l, display: grid, itemWidth: 120, minColumns: 2, maxColumns: 4, direction: rtl-snake, item: { type: text, bind: a } }\n";
    let spec = ok(&screen(grid));
    let list = &spec.tree["screens"]["main"]["layout"]["children"][0];
    assert_eq!(list["display"], "grid");
    assert_eq!(list["direction"], "rtl-snake");
    assert_eq!(list["maxColumns"], 4);
    single_error(&screen("    - { type: list, id: l, maxColumns: 3, item: { type: text, bind: a } }\n"), 4, "'maxColumns' n'a de sens qu'avec 'display: grid'");
    single_error(&screen("    - { type: list, id: l, display: grid, minColumns: 5, maxColumns: 2, item: { type: text, bind: a } }\n"), 4, "'minColumns' (5) doit être inférieur ou égal à 'maxColumns' (2)");
    single_error(&screen("    - { type: list, id: l, display: grid, direction: snake, item: { type: text, bind: a } }\n"), 4, "direction");
}

#[test]
fn theme_style_overrides() {
    let source = "styles:\n  title: { bold: true, color: \"#000000\" }\n  dark:\n    title: { color: \"#ffffff\" }\n  light:\n    only_light: { italic: true }\n".to_string()
        + &screen("    - { type: text, value: a, style: title }\n    - { type: text, value: b, style: only_light }\n");
    let spec = ok(&source);
    assert_eq!(spec.tree["styles"]["title"]["bold"], true);
    assert!(spec.tree["styles"].get("dark").is_none(), "{}", spec.tree["styles"]);
    assert_eq!(spec.tree["themeStyles"]["dark"]["title"]["color"], "#ffffff");
    assert!(spec.tree["themeStyles"]["light"]["only_light"].is_object());
    let bad = "styles:\n  dark:\n    title: { colour: \"#ffffff\" }\n".to_string() + &screen("    - { type: text, value: a }\n");
    single_error(&bad, 3, "colour");
}

const CARD: &str = "components:\n  carCard:\n    params:\n      title: text\n      photo: { type: text, required: false }\n      tone: { type: color, default: \"#123\" }\n      onOpen: action\n    layout:\n      type: card\n      onClick: $onOpen\n      children:\n        - { type: image, id: photo, src: \"https://x/p.png\", tooltip: $photo, label: x }\n        - { type: text, id: title, value: \"Voiture : $title\", color: $tone }\n";

#[test]
fn component_own_actions_are_scoped() {
    let source = "components:\n  carCard:\n    params: { title: text, onOpen: action }\n    layout:\n      type: card\n      onClick: $onOpen\n      children:\n        - { type: text, value: $title }\n        - { type: button, text: \"Suppr\", onClick: self.remove }\n        - { type: button, text: \"Voir\", onClick: self.open }\n".to_string()
        + "layout:\n  type: column\n  children:\n    - { type: carCard, id: best, title: x, onOpen: open_best }\n    - { type: carCard, title: \"self.accueil\", onOpen: open_y }\n";
    let spec = ok(&source);
    let best = &spec.tree["screens"]["main"]["layout"]["children"][0];
    assert_eq!(best["children"][1]["onClick"], "carCard.remove");
    // Nœud qui porte une action propre sans `id` : `id` de l'action, préfixé.
    assert_eq!(best["children"][1]["id"], "best.remove");
    assert_eq!(best["children"][2]["id"], "best.open");
    assert!(spec.actions.contains("carCard.remove") && spec.actions.contains("open_best"));
    // Texte d'instance qui commence par `self.` : un texte ; sans `id` d'instance, pas d'`id` ajouté.
    let second = &spec.tree["screens"]["main"]["layout"]["children"][1];
    assert_eq!(second["children"][0]["value"], "self.accueil");
    assert!(second["children"][1].get("id").is_none(), "{}", second);
    assert_eq!(spec.tree["components"]["carCard"], serde_json::json!(["open", "remove"]));
    assert_eq!(spec.tree["instances"]["best"], "carCard");
    assert!(spec.tree["instances"].get("second").is_none());
}

#[test]
fn components_are_expanded() {
    let source = CARD.to_string() + "layout:\n  type: column\n  children:\n    - { type: carCard, id: a, title: Clio, photo: \"https://x/a.png\", onOpen: open_a, width: 200 }\n    - { type: carCard, id: b, title: Polo, onOpen: open_b, tone: \"rgb(255, 0, 0)\" }\n";
    let spec = ok(&source);
    let children = &spec.tree["screens"]["main"]["layout"]["children"];
    let a = &children[0];
    assert_eq!(a["type"], "card");
    assert_eq!(a["id"], "a");
    assert_eq!(a["width"], 200.0);
    assert_eq!(a["onClick"], "open_a");
    assert_eq!(a["children"][0]["id"], "a.photo");
    assert_eq!(a["children"][1]["value"], "Voiture : Clio");
    assert_eq!(a["children"][1]["color"], "#112233");
    // `photo` absent : la clé qui le cite est retirée.
    assert_eq!(a["children"][0]["tooltip"], "https://x/a.png");
    let b = &children[1];
    assert!(b["children"][0].get("tooltip").is_none(), "{}", b["children"][0]);
    assert_eq!(b["children"][1]["id"], "b.title");
    assert_eq!(b["children"][1]["color"], "#ff0000");
    assert_eq!(spec.ids["b.title"], "text");
    assert!(spec.actions.contains("open_b"));
}

#[test]
fn component_errors() {
    let screen = |c: &str| CARD.to_string() + "layout:\n  type: column\n  children:\n" + c;
    let found = errors(&screen("    - { type: carCard, id: a, onOpen: go }\n"));
    assert!(found.iter().any(|(_, m)| m.contains("paramètre 'title' obligatoire")), "{found:?}");
    let found = errors(&screen("    - { type: carCard, id: a, title: x, onOpen: \"pas une action\" }\n"));
    assert!(found.iter().any(|(_, m)| m.contains("paramètre 'onOpen' de 'carCard'")), "{found:?}");
    // Deux instances sans `id` : les `id` internes se répètent.
    let found = errors(&screen("    - { type: carCard, title: x, onOpen: go }\n    - { type: carCard, title: y, onOpen: go }\n"));
    assert!(found.iter().any(|(_, m)| m.contains("id 'photo' déjà utilisé")), "{found:?}");
    let found = errors(&screen("    - { type: carCrd, title: x, onOpen: go }\n"));
    assert!(found.iter().any(|(_, m)| m.contains("vouliez-vous dire 'carCard'")), "{found:?}");
    let bad = "components:\n  c:\n    params: { a: txt }\n    layout: { type: text, value: $b }\n".to_string() + &super_screen();
    let found = errors(&bad);
    assert!(found.iter().any(|(_, m)| m.contains("type 'txt' inconnu")), "{found:?}");
    assert!(found.iter().any(|(_, m)| m.contains("'$b' n'est pas un paramètre")), "{found:?}");
    let cycle = "components:\n  loop:\n    layout: { type: column, children: [{ type: loop }] }\n".to_string()
        + "layout:\n  type: column\n  children:\n    - { type: loop }\n";
    assert!(errors(&cycle).iter().any(|(_, m)| m.contains("utilisé dans lui-même")));
}

fn super_screen() -> String {
    "layout:\n  type: column\n  children: []\n".to_string()
}

#[test]
fn component_params_bound_in_list_items() {
    let source = "components:\n  line:\n    params: { label: text }\n    layout: { type: text, value: $label }\n".to_string()
        + "layout:\n  type: list\n  id: l\n  item: { type: line, label: { bind: name } }\n";
    let spec = ok(&source);
    let item = &spec.tree["screens"]["main"]["layout"]["item"];
    assert_eq!(item["type"], "text");
    assert_eq!(item["binds"]["value"], "name");
    let outside = "components:\n  line:\n    params: { label: text }\n    layout: { type: text, value: $label }\n".to_string()
        + "layout: { type: line, label: { bind: name } }\n";
    assert!(errors(&outside).iter().any(|(_, m)| m.contains("'bind' n'est permis que dans le gabarit")));
}

#[test]
fn components_from_included_files_locate_their_errors() {
    use crate::core::ui_schema::check_file;
    let main = "styles:\n  base: { bold: true }\ncomponents:\n  includes:\n    - ui.d/parts.oc.ui\nlayout:\n  type: column\n  children:\n    - { type: badge, text: \"Neuf\" }\n";
    let parts = "styles:\n  badge_style: { color: \"#fff\" }\ncomponents:\n  badge:\n    params: { text: text }\n    layout: { type: text, value: $text, style: badge_style }\n";
    let dir = files("comp_inc", &[("main.oc.ui", main), ("ui.d/parts.oc.ui", parts)]);
    let spec = check_file(&dir.join("main.oc.ui")).expect("arbre attendu");
    let badge = &spec.tree["screens"]["main"]["layout"]["children"][0];
    assert_eq!(badge["value"], "Neuf");
    assert_eq!(spec.tree["styles"]["badge_style"]["color"], "#ffffff");
    let broken = "components:\n  badge:\n    params: { text: text }\n    layout: { type: text, value: $text, colour: red }\n";
    let dir = files("comp_inc_err", &[("main.oc.ui", main), ("ui.d/parts.oc.ui", broken)]);
    let errors = check_file(&dir.join("main.oc.ui")).unwrap_err();
    std::fs::remove_dir_all(&dir).ok();
    assert!(errors.iter().any(|e| e.file.ends_with("ui.d/parts.oc.ui") && e.line == 4 && e.message.contains("colour")), "{errors:?}");
}

#[test]
fn variables_in_texts() {
    let spec = ok(&screen("    - { type: text, value: \"${title} (${count})\", tooltip: \"$${pas_une}\" }\n"));
    let vars: Vec<&str> = spec.vars.iter().map(String::as_str).collect();
    assert_eq!(vars, ["count", "title"]);
    assert_eq!(spec.tree["vars"][0], "count");
    single_error(&screen("    - { type: text, value: \"${titre\" }\n"), 4, "sans '}' fermant");
    single_error(&screen("    - { type: text, value: \"${mon titre}\" }\n"), 4, "variable '${mon titre}' invalide");
}

#[test]
fn size_variants_are_embedded() {
    use crate::core::ui_schema::check_interface;
    let base = "layout: { type: column, children: [{ type: text, id: t, value: a }] }\n";
    let md = "layout: { type: row, children: [{ type: text, id: t, value: b }, { type: input, id: extra }] }\n";
    let dir = files("variants", &[("g.oc.ui", base), ("g.md.oc.ui", md)]);
    let spec = check_interface(&dir.join("g.oc.ui")).expect("arbre attendu");
    assert_eq!(spec.tree["variants"]["md"]["screens"]["main"]["layout"]["type"], "row");
    assert_eq!(spec.tree["sizes"]["md"], 840);
    assert_eq!(spec.ids["extra"], "input");

    let conflict = "layout: { type: column, children: [{ type: input, id: t }] }\n";
    let dir = files("variants_conflict", &[("g.oc.ui", base), ("g.md.oc.ui", conflict)]);
    let errors = check_interface(&dir.join("g.oc.ui")).unwrap_err();
    std::fs::remove_dir_all(&dir).ok();
    assert!(errors[0].message.contains("id 't'") && errors[0].file.ends_with("g.md.oc.ui"), "{:?}", errors);

    // Sans variante : seuils quand même présents (ui.sizeClass()).
    let dir = files("variants_none", &[("g.oc.ui", base)]);
    let spec = check_interface(&dir.join("g.oc.ui")).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(spec.tree["sizes"]["xs"], 360);
    assert!(spec.tree.get("variants").is_none());
}

#[test]
fn android_material_nodes() {
    ok(&screen("    - { type: android.fab, icon: add, label: Ajouter, text: Ajouter, onClick: add }\n    - { type: android.drawer, id: menu, children: [{ type: text, value: Menu }] }\n    - { type: android.bottomSheet, id: sheet, open: false, onDismiss: closed, children: [] }\n"));
    single_error(&screen("    - { type: android.fab, icon: add }\n"), 4, "'android.fab' : propriété 'label' obligatoire");
    single_error(&screen("    - { type: android.drawer, children: [] }\n"), 4, "'android.drawer' : propriété 'id' obligatoire");
}
