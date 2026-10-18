//! Tests — racines du projet sans `workspaceFolders` : l'index restait vide,
//! un renommage ne modifiait que la déclaration (voir
//! docs/roadmap.d/tooling-language-server.md).

use std::path::PathBuf;

use lsp_types::{InitializeParams, Url};

use super::client_roots;
use super::project::ProjectIndex;
use super::workspace::Workspace;

fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ocara_lsp_roots_{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(dir.join("main.oc"), "architecture permissive\nimport lib.Tools\nfunction main(): int {\n    return Tools::twice(2)\n}\n").unwrap();
    std::fs::write(dir.join("lib/Tools.oc"), "namespace lib\nclass Tools {\n    public static method twice(n:int): int {\n        return n * 2\n    }\n}\n").unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn root_uri_is_used_without_workspace_folders() {
    let dir = project("uri");
    #[allow(deprecated)]
    let params = InitializeParams { root_uri: Some(Url::from_directory_path(&dir).unwrap()), ..Default::default() };
    assert_eq!(client_roots(&params), vec![dir.clone()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opened_file_outside_roots_adopts_its_project() {
    let dir = project("adopt");
    let tools = dir.join("lib/Tools.oc");
    let mut ws = Workspace::new(Vec::new());
    ws.open(tools.clone(), std::fs::read_to_string(&tools).unwrap());

    assert!(ws.adopt_root(&tools));
    assert_eq!(ws.roots(), [dir.clone()].as_slice());
    assert!(!ws.adopt_root(&tools));

    let mut index = ProjectIndex::default();
    index.ensure(&ws);
    let files = index.files();
    assert!(files.contains(&dir.join("main.oc")), "{:?}", files);
    assert!(files.contains(&tools), "{:?}", files);
    let _ = std::fs::remove_dir_all(&dir);
}
