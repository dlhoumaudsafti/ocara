/// Tests unitaires — types homonymes de namespaces différents (voir
/// docs/roadmap.d/langage-classes-homonymes-fusionnees.md) : chacun reçoit
/// un nom interne, chaque fichier utilise celui qu'il importe.

use crate::core::analysis::{analyze, homonyms, AnalyzeOptions};
use crate::parsing::ast::{ClassMember, Expr, Stmt};

fn write(dir: &std::path::Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Classe appelée par `Helper::label()` dans la méthode `run` de `class`.
fn helper_called_by(program: &crate::parsing::ast::Program, class: &str) -> String {
    let c = program.classes.iter().find(|c| c.name == class).unwrap();
    let body = c.members.iter().find_map(|m| match m { ClassMember::Method { decl, .. } => Some(&decl.body), _ => None }).unwrap();
    match &body.stmts[0] {
        Stmt::Return { value: Some(Expr::StaticCall { class, .. }), .. } => class.clone(),
        other => panic!("return Helper::label() attendu, trouvé {:?}", other),
    }
}

#[test]
fn homonymous_classes_resolve_per_importing_file() {
    let dir = std::env::temp_dir().join(format!("ocara_homonyms_{}", std::process::id()));
    let helper = |ns: &str| format!("namespace {}\n\nclass Helper {{\n    public static method label(): string {{\n        return \"{}\"\n    }}\n}}\n", ns, ns);
    write(&dir, "a/Helper.oc", &helper("a"));
    write(&dir, "b/Helper.oc", &helper("b"));
    let user = |ns: &str, class: &str| format!("namespace {}\n\nimport {}.Helper\n\nclass {} {{\n    public static method run(): string {{\n        return Helper::label()\n    }}\n}}\n", ns, ns, class);
    write(&dir, "a/UseA.oc", &user("a", "UseA"));
    write(&dir, "b/UseB.oc", &user("b", "UseB"));
    write(&dir, "main.oc", "architecture permissive\nimport ocara.IO\nimport a.UseA\nimport b.UseB\n\nfunction main(): int {\n    IO::writeln(UseA::run() + UseB::run())\n    return 0\n}\n");

    let input = dir.join("main.oc");
    let analysis = analyze(&AnalyzeOptions { input: &input, src_dir: Some(&dir), dump: false, index: false, tolerant: false, architecture_only: false });
    std::fs::remove_dir_all(&dir).ok();

    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    let program = &analysis.checked.unwrap().program;
    let helpers: Vec<&str> = program.classes.iter().map(|c| c.name.as_str()).filter(|n| n.starts_with("Helper")).collect();
    assert_eq!(helpers.len(), 2, "{:?}", helpers);
    assert_eq!(helper_called_by(program, "UseA"), "Helper__ns__a");
    assert_eq!(helper_called_by(program, "UseB"), "Helper__ns__b");
}

#[test]
fn internal_names_are_hidden_in_messages() {
    assert_eq!(homonyms::display("undefined symbol 'Format__ns__context_car::date'"), "undefined symbol 'Format::date'");
    assert_eq!(homonyms::display("aucun nom interne"), "aucun nom interne");
}

#[test]
fn homonymous_functions_resolve_per_importing_file() {
    let dir = std::env::temp_dir().join(format!("ocara_homonym_fns_{}", std::process::id()));
    let util = |ns: &str| format!("namespace {}\n\nfunction label(): string {{\n    return \"{}\"\n}}\n", ns, ns);
    write(&dir, "a/util.oc", &util("a"));
    write(&dir, "b/util.oc", &util("b"));
    let user = |ns: &str, class: &str| format!("namespace {}\n\nimport label from \"util\"\n\nclass {} {{\n    public static method run(): string {{\n        return label()\n    }}\n}}\n", ns, class);
    write(&dir, "a/UseA.oc", &user("a", "UseA"));
    write(&dir, "b/UseB.oc", &user("b", "UseB"));
    write(&dir, "main.oc", "architecture permissive\nimport ocara.IO\nimport a.UseA\nimport b.UseB\n\nfunction main(): int {\n    IO::writeln(UseA::run() + UseB::run())\n    return 0\n}\n");

    let input = dir.join("main.oc");
    let analysis = analyze(&AnalyzeOptions { input: &input, src_dir: Some(&dir), dump: false, index: false, tolerant: false, architecture_only: false });
    std::fs::remove_dir_all(&dir).ok();

    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    let program = &analysis.checked.unwrap().program;
    let called = |class: &str| {
        let c = program.classes.iter().find(|c| c.name == class).unwrap();
        let body = c.members.iter().find_map(|m| match m { ClassMember::Method { decl, .. } => Some(&decl.body), _ => None }).unwrap();
        match &body.stmts[0] {
            Stmt::Return { value: Some(Expr::Call { callee, .. }), .. } => match callee.as_ref() {
                Expr::Ident(name, _) => name.clone(),
                other => panic!("appel de fonction attendu, trouvé {:?}", other),
            },
            other => panic!("return label() attendu, trouvé {:?}", other),
        }
    };
    assert_eq!(called("UseA"), "label__ns__a");
    assert_eq!(called("UseB"), "label__ns__b");
    assert!(program.functions.iter().any(|f| f.name == "main"));
}

/// Une variable locale ou un paramètre portant le nom d'une fonction
/// homonyme la masque : l'appel n'est pas réécrit vers la fonction globale.
#[test]
fn local_names_shadow_homonymous_functions() {
    let dir = std::env::temp_dir().join(format!("ocara_homonym_shadow_{}", std::process::id()));
    let util = |ns: &str| format!("namespace {}\n\nfunction label(): string {{\n    return \"{}\"\n}}\n", ns, ns);
    write(&dir, "a/util.oc", &util("a"));
    write(&dir, "b/util.oc", &util("b"));
    let user = |ns: &str, class: &str| format!("namespace {}\n\nimport label from \"util\"\n\nclass {} {{\n    public static method run(): string {{\n        var label:Function<string()> = nameless(): string {{ return \"LOCAL\" }}\n        return label()\n    }}\n    public static method param(label:string): string {{\n        return label\n    }}\n}}\n", ns, class);
    write(&dir, "a/UseA.oc", &user("a", "UseA"));
    write(&dir, "b/UseB.oc", &user("b", "UseB"));
    write(&dir, "main.oc", "architecture permissive\nimport ocara.IO\nimport a.UseA\nimport b.UseB\n\nfunction main(): int {\n    IO::writeln(UseA::run() + UseB::param(\"p\"))\n    return 0\n}\n");

    let input = dir.join("main.oc");
    let analysis = analyze(&AnalyzeOptions { input: &input, src_dir: Some(&dir), dump: false, index: false, tolerant: false, architecture_only: false });
    std::fs::remove_dir_all(&dir).ok();

    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    let program = &analysis.checked.unwrap().program;
    let c = program.classes.iter().find(|c| c.name == "UseA").unwrap();
    let body = c.members.iter().find_map(|m| match m { ClassMember::Method { decl, .. } if decl.name == "run" => Some(&decl.body), _ => None }).unwrap();
    match &body.stmts[1] {
        Stmt::Return { value: Some(Expr::Call { callee, .. }), .. } => match callee.as_ref() {
            Expr::Ident(name, _) => assert_eq!(name, "label"),
            other => panic!("appel de la variable locale attendu, trouvé {:?}", other),
        },
        other => panic!("return label() attendu, trouvé {:?}", other),
    }
}
