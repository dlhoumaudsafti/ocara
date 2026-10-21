/// Tests unitaires — `core::leak_sites` : lieu de déclaration du `main`
/// synthétisé des blocs runtime (bilan `--leak-check`) — sa ligne était 0
/// (docs/roadmap.d/langage-runtime-blocs-variables.md, « Reste »).
use crate::core::analysis::{analyze, AnalyzeOptions};
use crate::core::leak_sites::sites;

#[test]
fn runtime_main_is_located_at_its_declaration() {
    let dir = std::env::temp_dir().join(format!("ocara_leak_sites_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(dir.join("core")).unwrap();
    std::fs::write(dir.join("core/start.runtime.oc"), "IO::writeln(\"bonjour\")\n").unwrap();
    std::fs::write(dir.join("main.oc"), "architecture permissive\nimport ocara.IO\n\nruntime core.start is main\n").unwrap();
    let input = dir.join("main.oc");
    let analysis = analyze(&AnalyzeOptions { input: &input, src_dir: Some(&dir), dump: false, index: false, tolerant: false, architecture_only: false });
    let program = analysis.checked.expect("analyse sans erreur").program;
    let found = sites(&program, &input);
    std::fs::remove_dir_all(&dir).ok();
    let (file, line) = found.get("main").expect("main runtime");
    assert_eq!(*line, 4);
    assert!(file.ends_with("main.oc"), "{}", file);
}
