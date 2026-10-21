/// Tests unitaires — `core::cli::parse` : erreurs explicites au lieu de
/// replis silencieux (docs/roadmap.d/tooling-cli-compilateur.md).
use std::path::{Path, PathBuf};
use crate::core::cli::{object_path, parse};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn output_equal_to_source_is_refused() {
    let err = parse(&args(&["x.oc", "-o", "x.oc"])).err().expect("refus attendu");
    assert!(err.contains("écrasé"), "{}", err);
    assert!(parse(&args(&["x.oc", "-o", "./x.oc"])).is_err());
    assert!(parse(&args(&["x.oc", "-o", "x"])).is_ok());
}

#[test]
fn missing_values_unknown_options_and_extra_inputs_are_errors() {
    assert!(parse(&args(&["x.oc", "-o"])).err().unwrap().contains("-o nécessite une valeur"));
    assert!(parse(&args(&["x.oc", "--src"])).err().unwrap().contains("--src"));
    assert!(parse(&args(&["x.oc", "--chek"])).err().unwrap().contains("option inconnue : --chek"));
    assert!(parse(&args(&["a.oc", "b.oc"])).err().unwrap().contains("un seul fichier"));
    assert!(parse(&args(&["--check"])).err().unwrap().contains("aucun fichier"));
}

#[test]
fn historical_build_word_and_lsp_without_input_are_accepted() {
    let cli = parse(&args(&["build", "main.oc", "-o", "out"])).expect("ocara build main.oc");
    assert_eq!(cli.input, PathBuf::from("main.oc"));
    assert!(parse(&args(&["--lsp"])).expect("--lsp seul").lsp);
}

#[test]
fn object_file_appends_its_extension() {
    assert_eq!(object_path(Path::new("build/app.1.2")), PathBuf::from("build/app.1.2.o"));
    assert_eq!(object_path(Path::new("out")), PathBuf::from("out.o"));
}
