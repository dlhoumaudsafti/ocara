/// Tests unitaires — `use UI("x.oc.ui")` (src/core/ui_file.rs) : l'argument
/// devient l'arbre JSON, une erreur du `.oc.ui` est située dans ce fichier.

use crate::core::ui_file::desugar_new;
use crate::parsing::ast::{Expr, Literal};
use crate::parsing::token::Span;

/// Dossier temporaire avec `x.oc.ui` ; `use` situé dans `<dossier>/main.oc`.
fn desugar(name: &str, ui: &str) -> (Result<Vec<Expr>, (Span, String)>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ocara_ui_file_{}_{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("x.oc.ui"), ui).unwrap();
    let mut span = Span::new(4, 21);
    span.file = Some(dir.join("main.oc").display().to_string());
    let mut args = vec![Expr::Literal(Literal::String("x.oc.ui".into()), span.clone())];
    let result = desugar_new(&mut args, &span, Some("x.oc.ui".into())).map(|_| args);
    (result, dir)
}

#[test]
fn path_becomes_the_json_tree() {
    let (result, dir) = desugar("ok", "layout:\n  type: column\n  children:\n    - { type: button, text: Ok, onClick: save }\n");
    std::fs::remove_dir_all(&dir).ok();
    let args = result.expect("arbre attendu");
    let Expr::Literal(Literal::String(json), _) = &args[0] else { panic!("littéral attendu") };
    let tree: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(tree["start"], "main");
    assert_eq!(tree["actions"][0], "save");
}

#[test]
fn ui_error_points_into_the_ui_file() {
    let (result, dir) = desugar("err", "layout:\n  type: column\n  children:\n    - { type: buton }\n    - { type: nope }\n");
    std::fs::remove_dir_all(&dir).ok();
    let (span, message) = result.expect_err("erreur attendue");
    assert_eq!((span.line, span.col), (4, 15));
    assert!(span.file.as_deref().unwrap().ends_with("x.oc.ui"));
    assert!(message.contains("'buton' inconnu") && message.contains("1 autre(s) erreur(s)"), "{message}");
}

#[test]
fn missing_file_and_unknown_path() {
    let mut span = Span::new(1, 1);
    span.file = Some("/nulle/part/main.oc".into());
    let mut args = vec![Expr::Literal(Literal::String("absent.oc.ui".into()), span.clone())];
    let (_, message) = desugar_new(&mut args, &span, Some("absent.oc.ui".into())).expect_err("fichier absent");
    assert!(message.contains("lecture de '/nulle/part/absent.oc.ui'"), "{message}");
    let (_, message) = desugar_new(&mut args, &span, None).expect_err("chemin inconnu");
    assert!(message.contains("connu à la compilation"), "{message}");
}

/// `ui.on`/`get`/`setItems`/`navigate` avec un littéral : vérifiés contre l'arbre.
#[test]
fn calls_are_checked_against_the_tree() {
    use crate::core::ui_file::check_call;
    let tree = r#"{"actions":["search"],"ids":{"q":"input","cars":"list","web":"webview"},"screens":{"home":{},"details":{}},"components":{"carCard":["open"]}}"#;
    let span = Span::new(7, 5);
    let lit = |s: &str| vec![Expr::Literal(Literal::String(s.into()), span.clone())];
    assert!(check_call(tree, "on", &lit("search"), &span).is_ok());
    assert!(check_call(tree, "on", &lit("back"), &span).is_ok());
    let (at, message) = check_call(tree, "on", &lit("serch"), &span).unwrap_err();
    assert_eq!(at.line, 7);
    assert!(message.contains("action 'serch'") && message.contains("'search'"), "{message}");
    assert!(check_call(tree, "get", &lit("q"), &span).is_ok());
    assert!(check_call(tree, "set", &lit("nope"), &span).unwrap_err().1.contains("id 'nope'"));
    assert!(check_call(tree, "has", &lit("q"), &span).is_ok());
    assert!(check_call(tree, "component", &lit("carCard"), &span).is_ok());
    assert!(check_call(tree, "component", &lit("carCrd"), &span).unwrap_err().1.contains("vouliez-vous dire 'carCard'"));
    assert!(check_call(tree, "has", &lit("nope"), &span).unwrap_err().1.contains("ui.has : id 'nope'"));
    assert!(check_call(tree, "setItems", &lit("q"), &span).unwrap_err().1.contains("'list' attendu"));
    assert!(check_call(tree, "eval", &lit("web"), &span).is_ok());
    assert!(check_call(tree, "navigate", &lit("detail"), &span).unwrap_err().1.contains("'details'"));
    // Argument non littéral : vérifié à l'exécution.
    let var = vec![Expr::Ident("x".into(), span.clone())];
    assert!(check_call(tree, "get", &var, &span).is_ok());
}
