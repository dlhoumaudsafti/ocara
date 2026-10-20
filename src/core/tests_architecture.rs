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
    let analysis = analyze(&AnalyzeOptions { input: &input, src_dir: Some(&dir), dump: false, index: false, tolerant: false, architecture_only: false });
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

/// Projet dans `<tmp>/proj` (fichier d'entrée `proj/main.oc`), fichiers
/// placés relativement à `<tmp>` — pour une bibliothèque hors du projet.
fn layout_errors(test: &str, files: &[(&str, String)], main: &str) -> Vec<String> {
    let base = std::env::temp_dir().join(format!("ocara_arch_layout_{}_{}", test, std::process::id()));
    std::fs::remove_dir_all(&base).ok();
    for (rel, text) in files {
        write(&base, rel, text);
    }
    write(&base, "proj/main.oc", main);
    let input = base.join("proj/main.oc");
    let src_dir = base.join("proj");
    let analysis = analyze(&AnalyzeOptions { input: &input, src_dir: Some(&src_dir), dump: false, index: false, tolerant: false, architecture_only: false });
    std::fs::remove_dir_all(&base).ok();
    analysis.diagnostics.iter().filter(|d| d.is_error()).map(|d| d.message.clone()).collect()
}

const LIB: &str = "class Util {\n    public static method ping(): int {\n        return 1\n    }\n}\n";

fn relative_import_class(ns: &str, name: &str, from: &str) -> String {
    format!("namespace {}\n\nimport Util from \"{}\"\n\nclass {} {{\n    public static method ping(): int {{\n        return Util::ping()\n    }}\n}}\n", ns, from, name)
}

/// Bibliothèque hors du projet : importable depuis l'infrastructure, pas
/// depuis le domaine ni l'application (décision du 2026-10-13).
#[test]
fn external_library_only_from_infrastructure() {
    let main = format!("import ocara.IO\nimport context.home.infra.Repo\n\n{}\n\nfunction main(): int {{\n    return 0\n}}\n", HEXA);
    let files = [
        ("lib/Util.oc", LIB.to_string()),
        ("proj/context/home/infra/Repo.oc", relative_import_class("context.home.infra", "Repo", "../../../../lib/Util")),
    ];
    let errs = layout_errors("extok", &files, &main);
    assert!(!errs.iter().any(|e| e.contains("bibliothèque externe")), "{:?}", errs);

    let main = format!("import ocara.IO\nimport context.home.domain.Entity\n\n{}\n\nfunction main(): int {{\n    return 0\n}}\n", HEXA);
    let files = [
        ("lib/Util.oc", LIB.to_string()),
        ("proj/context/home/domain/Entity.oc", relative_import_class("context.home.domain", "Entity", "../../../../lib/Util")),
    ];
    let errs = layout_errors("extko", &files, &main);
    assert!(errs.iter().any(|e| e.contains("ne peut pas importer la bibliothèque externe")), "{:?}", errs);
}

/// Un second répertoire hexagonal dans le graphe est refusé : une seule
/// racine par programme (décision du 2026-10-13).
#[test]
fn single_hexagonal_root() {
    let files = [
        ("vendor/context/home/domain/Other.oc", class("vendor.context.home.domain", "Other", &[])),
        ("context/home/infra/Repo.oc", class("context.home.infra", "Repo", &["vendor.context.home.domain.Other"])),
    ];
    let errs = errors("tworoots", HEXA, &files, &["context.home.infra.Repo"]);
    assert!(errs.iter().any(|e| e.contains("second répertoire hexagonal 'vendor/context'")), "{:?}", errs);
}

/// Imports relatifs et transitifs : chaque fichier du graphe est vérifié.
#[test]
fn relative_and_transitive_imports_are_checked() {
    let files = [
        ("context/home/infra/Repo.oc", class("context.home.infra", "Repo", &[])),
        ("context/home/app/Service.oc", "namespace context.home.app\n\nimport Repo from \"../infra/Repo\"\n\nclass Service {\n    public static method ping(): int {\n        return 1\n    }\n}\n".to_string()),
        ("context/home/domain/Entity.oc", class("context.home.domain", "Entity", &[])),
        ("context/home/infra/Entry.oc", class("context.home.infra", "Entry", &["context.home.app.Service", "context.home.domain.Entity"])),
    ];
    let errs = errors("relative", HEXA, &files, &["context.home.infra.Entry"]);
    assert!(errs.iter().any(|e| e.contains("application (app) peut dépendre de application et domain, pas de infrastructure")), "{:?}", errs);
}

/// Un fichier de test `ocaraunit` suit la configuration du `main.oc` du
/// projet : une violation dans son graphe est rejetée, sa propre
/// `architecture permissive` la lève (plan de tests, point 10).
#[test]
fn test_files_follow_project_architecture() {
    let run = |test: &str, test_header: &str| -> Vec<String> {
        let dir = std::env::temp_dir().join(format!("ocara_arch_unit_{}_{}", test, std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        write(&dir, "main.oc", &format!("import ocara.IO\n\n{}\n\nfunction main(): int {{\n    return 0\n}}\n", HEXA));
        write(&dir, "context/home/app/Service.oc", &class("context.home.app", "Service", &[]));
        write(&dir, "context/home/domain/Entity.oc", &class("context.home.domain", "Entity", &["context.home.app.Service"]));
        write(&dir, "tests/EntityTest.oc", &format!("{}\nimport ocara.UnitTest\nimport context.home.domain.Entity\n\nclass Test_Entity {{\n    public method pingTest(): int {{\n        UnitTest::assertEquals(1, Entity::ping())\n        return 0\n    }}\n}}\n", test_header));
        let input = dir.join("tests/EntityTest.oc");
        let analysis = analyze(&AnalyzeOptions { input: &input, src_dir: Some(&dir), dump: false, index: false, tolerant: false, architecture_only: false });
        std::fs::remove_dir_all(&dir).ok();
        analysis.diagnostics.iter().filter(|d| d.is_error()).map(|d| d.message.clone()).collect()
    };
    let errs = run("strict", "");
    assert!(errs.iter().any(|e| e.contains("domain (domain) peut dépendre de domain, pas de application")), "{:?}", errs);
    let errs = run("permissive", "architecture permissive\n");
    assert!(!errs.iter().any(|e| e.contains("architecture hexagonale")), "{:?}", errs);
}

/// `--check-architecture` : seuls les diagnostics d'architecture comptent
/// (une erreur de type du programme n'en est pas un).
#[test]
fn architecture_only_reports_architecture_errors_only() {
    let check = |test: &str, entity_imports: &[&str]| -> Vec<String> {
        let dir = std::env::temp_dir().join(format!("ocara_arch_only_{}_{}", test, std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        write(&dir, "context/home/app/Service.oc", &class("context.home.app", "Service", &[]));
        write(&dir, "context/home/domain/Entity.oc", &class("context.home.domain", "Entity", entity_imports));
        write(&dir, "main.oc", &format!("import context.home.domain.Entity\n{}\n\nfunction main(): int {{\n    var x:int = \"pas un entier\"\n    return 0\n}}\n", HEXA));
        let input = dir.join("main.oc");
        let analysis = analyze(&AnalyzeOptions { input: &input, src_dir: Some(&dir), dump: false, index: false, tolerant: false, architecture_only: true });
        std::fs::remove_dir_all(&dir).ok();
        analysis.diagnostics.iter().filter(|d| d.is_error()).map(|d| d.message.clone()).collect()
    };
    assert!(check("ok", &[]).is_empty(), "{:?}", check("ok", &[]));
    let errs = check("ko", &["context.home.app.Service"]);
    assert!(errs.iter().any(|e| e.contains("pas de application")), "{:?}", errs);
}
