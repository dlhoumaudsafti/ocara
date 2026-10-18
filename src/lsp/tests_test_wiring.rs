//! Tests — `wiring` d'une classe de test (EBNF §17.2) dans le serveur :
//! diagnostics, définition sur la ligne `wiring`, renommage de la classe
//! visée (la ligne `wiring` du test suit).

use std::path::{Path, PathBuf};

use lsp_types::{DocumentChangeOperation, DocumentChanges, OneOf, Position, Url, WorkspaceEdit};

use super::features::definition;
use super::navigation::rename;
use super::project::ProjectIndex;
use super::workspace::Workspace;

const TEST_FILE: &str = "tests/RepoTest.oc";
const FAKE_FILE: &str = "tests/support/FakeRepo.oc";

fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ocara_lsp_test_wiring_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["lib", "tests/support"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    let files = [
        ("main.oc", "architecture permissive\nimport lib.Repo\nfunction main(): int {\n    return Repo::size()\n}\n"),
        ("lib/Repo.oc", "namespace lib\ninterface Repo {\n    wiring lib.RealRepo\n    public static method size(): int\n}\n"),
        ("lib/RealRepo.oc", "namespace lib\nimport lib.Repo\nclass RealRepo implements Repo {\n    public static method size(): int {\n        return 1\n    }\n}\n"),
        (FAKE_FILE, "import lib.Repo\nclass FakeRepo implements Repo {\n    public static method size(): int {\n        return 0\n    }\n}\n"),
        (TEST_FILE, "architecture permissive\nimport ocara.UnitTest\nimport lib.Repo\nclass RepoTest extends UnitTest {\n    wiring tests.support.FakeRepo\n    public method sizeTest(): int {\n        self::assertEquals(0, Repo::size())\n        return 0\n    }\n}\nfunction main(): int {\n    return 0\n}\n"),
    ];
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.canonicalize().unwrap()
}

fn open(ws: &mut Workspace, path: &Path) {
    ws.open(path.to_path_buf(), std::fs::read_to_string(path).unwrap());
    ws.analyze(path);
}

#[test]
fn test_file_with_class_wiring_has_no_error() {
    let dir = project("diag");
    let test = dir.join(TEST_FILE);
    let mut ws = Workspace::new(vec![dir.clone()]);
    open(&mut ws, &test);
    let analysis = ws.analysis(&test).unwrap();
    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn definition_on_class_wiring_line_opens_the_target() {
    let dir = project("def");
    let test = dir.join(TEST_FILE);
    let mut ws = Workspace::new(vec![dir.clone()]);
    open(&mut ws, &test);
    let location = definition(&ws, &test, &Position::new(4, 30)).expect("definition");
    assert_eq!(location.uri, Url::from_file_path(dir.join(FAKE_FILE)).unwrap());
    assert_eq!(location.range.start.line, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renaming_the_target_updates_the_class_wiring_line() {
    let dir = project("rename");
    let fake = dir.join(FAKE_FILE);
    let mut ws = Workspace::new(vec![dir.clone()]);
    open(&mut ws, &fake);
    let mut index = ProjectIndex::default();
    index.ensure(&ws);
    let edit = rename(&ws, &index, &fake, &Position::new(1, 8), "StubRepo").expect("rename");
    let in_test = edits_for(&edit, &Url::from_file_path(dir.join(TEST_FILE)).unwrap());
    assert!(in_test.iter().any(|(line, text)| *line == 4 && text == "StubRepo"), "{:?}", in_test);
    let _ = std::fs::remove_dir_all(&dir);
}

/// (ligne, texte) des modifications de `uri` — la classe porte le nom de son
/// fichier, le renommage passe donc par `document_changes` (fichier renommé).
fn edits_for(edit: &WorkspaceEdit, uri: &Url) -> Vec<(u32, String)> {
    let Some(DocumentChanges::Operations(operations)) = &edit.document_changes else { return Vec::new() };
    operations.iter()
        .filter_map(|op| match op {
            DocumentChangeOperation::Edit(e) if &e.text_document.uri == uri => Some(&e.edits),
            _ => None,
        })
        .flatten()
        .filter_map(|e| match e {
            OneOf::Left(e) => Some((e.range.start.line, e.new_text.clone())),
            OneOf::Right(_) => None,
        })
        .collect()
}
