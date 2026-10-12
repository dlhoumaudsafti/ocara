/// Tests unitaires — architecture hexagonale (voir
/// docs/roadmap.d/langage-mode-hexa.md et core/analysis.d/architecture.rs).

use crate::core::analysis::{analyze, AnalyzeOptions};

fn write(dir: &std::path::Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Classe vide `name` dans `ns`, avec ses imports.
fn class(ns: &str, name: &str, imports: &[&str]) -> String {
    let imports: String = imports.iter().map(|i| format!("import {}\n", i)).collect();
    format!("namespace {}\n\n{}\nclass {} {{\n    public static method ping(): int {{\n        return 1\n    }}\n}}\n", ns, imports, name)
}

/// Projet temporaire : `files` (chemin, contenu), puis `main.oc` qui
/// déclare `architecture` et importe `entry_imports`. Messages d'erreur.
fn errors(test: &str, architecture: &str, files: &[(&str, String)], entry_imports: &[&str]) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!("ocara_arch_{}_{}", test, std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    for (rel, text) in files {
        write(&dir, rel, text);
    }
    let imports: String = entry_imports.iter().map(|i| format!("import {}\n", i)).collect();
    write(&dir, "main.oc", &format!("import ocara.IO\n{}\n{}\n\nfunction main(): int {{\n    return 0\n}}\n", imports, architecture));
    let input = dir.join("main.oc");
    let analysis = analyze(&AnalyzeOptions { input: &input, src_dir: Some(&dir), dump: false, index: false, tolerant: false });
    std::fs::remove_dir_all(&dir).ok();
    analysis.diagnostics.iter().filter(|d| d.is_error()).map(|d| d.message.clone()).collect()
}

const HEXA: &str = "architecture hexagonal {\n    src: context,\n    layers: {\n        infrastructure: infra,\n        application: app,\n    },\n}";

#[test]
fn layers_point_inwards() {
    let files = [
        ("context/home/domain/Entity.oc", class("context.home.domain", "Entity", &[])),
        ("context/home/app/Service.oc", class("context.home.app", "Service", &["context.home.domain.Entity"])),
        ("context/home/infra/Repo.oc", class("context.home.infra", "Repo", &["context.home.app.Service", "context.home.domain.Entity"])),
    ];
    assert!(errors("ok", HEXA, &files, &["context.home.infra.Repo"]).is_empty());

    let files = [
        ("context/home/infra/Repo.oc", class("context.home.infra", "Repo", &[])),
        ("context/home/app/Service.oc", class("context.home.app", "Service", &["context.home.infra.Repo"])),
        ("context/home/domain/Entity.oc", class("context.home.domain", "Entity", &["context.home.app.Service"])),
    ];
    let errs = errors("inward", HEXA, &files, &["context.home.domain.Entity"]);
    assert!(errs.iter().any(|e| e.contains("context.home.domain.Entity") && e.contains("domain (domain) peut dépendre de domain, pas de application")), "{:?}", errs);
    assert!(errs.iter().any(|e| e.contains("application (app) peut dépendre de application et domain, pas de infrastructure")), "{:?}", errs);
}

#[test]
fn contexts_are_isolated_unless_shared() {
    let files = [
        ("context/car/domain/Car.oc", class("context.car.domain", "Car", &[])),
        ("context/home/app/Service.oc", class("context.home.app", "Service", &["context.car.domain.Car"])),
    ];
    let errs = errors("isolated", HEXA, &files, &["context.home.app.Service"]);
    assert!(errs.iter().any(|e| e.contains("contextes 'home' et 'car' sont isolés")), "{:?}", errs);

    let shared = "architecture hexagonal {\n    src: context,\n    layers: { infrastructure: infra, application: app },\n    shared: [context.car],\n}";
    assert!(errors("shared", shared, &files, &["context.home.app.Service"]).is_empty());
}

#[test]
fn nothing_outside_src_and_namespaces_follow_paths() {
    let files = [
        ("configs/Settings.oc", class("configs", "Settings", &[])),
        ("context/home/infra/Repo.oc", class("context.home.infra", "Repo", &["configs.Settings"])),
        ("context/home/app/Service.oc", class("context.wrong", "Service", &[])),
        ("context/Loose.oc", class("context", "Loose", &[])),
    ];
    let errs = errors("outside", HEXA, &files, &["context.home.infra.Repo", "context.home.app.Service", "context.Loose"]);
    assert!(errs.iter().any(|e| e.contains("hors du répertoire hexagonal 'context/'")), "{:?}", errs);
    assert!(errs.iter().any(|e| e.contains("'namespace context.home.app'")), "{:?}", errs);
    assert!(errs.iter().any(|e| e.contains("'context.Loose' n'est pas classable")), "{:?}", errs);
}

#[test]
fn configuration_errors() {
    let errs = errors("nosrc", "", &[], &[]);
    assert!(errs.iter().any(|e| e.contains("répertoire hexagonal 'src/' introuvable (architecture hexagonale par défaut)")), "{:?}", errs);
    assert!(errors("permissive", "architecture permissive", &[], &[]).is_empty());

    let files = [("configs/Settings.oc", class("configs", "Settings", &[])), ("context/home/app/S.oc", class("context.home.app", "S", &[]))];
    let errs = errors("sharedcfg", "architecture hexagonal {\n    src: context,\n    shared: [configs],\n}", &files, &[]);
    assert!(errs.iter().any(|e| e.contains("racine partagée 'configs' sans segment de couche")), "{:?}", errs);
}

#[test]
fn wiring_stays_in_its_context() {
    let files = [
        ("context/car/infra/CarRepo.oc", "namespace context.car.infra\n\nimport context.home.domain.Contract\n\nclass CarRepo implements Contract {\n    public static method ping(): int {\n        return 1\n    }\n}\n".to_string()),
        ("context/home/domain/Contract.oc", "namespace context.home.domain\n\ninterface Contract {\n    wiring context.car.infra.CarRepo\n    public static method ping(): int\n}\n".to_string()),
    ];
    let errs = errors("wiring", "architecture hexagonal {\n    src: context,\n    layers: { infrastructure: infra },\n}", &files, &["context.home.domain.Contract"]);
    assert!(errs.iter().any(|e| e.contains("'wiring context.car.infra.CarRepo' vise le contexte 'car'")), "{:?}", errs);
}
