//! Requête `ocara/workflow` (encart « Workflow » de l'extension), en
//! partant du script affiché : arbre des imports (runtimes compris),
//! structure (types, membres, variables), graphe d'appels et variantes
//! `when`. Chaque nœud : `label`, `description`, `kind` (icône), `file` et
//! `line` (1-based) pour l'ouvrir, `children`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::project;
use super::workspace::Workspace;
use crate::core::analysis::resolve_import_path;
use crate::core::runtime_expand::resolve_runtime_file;

pub const METHOD: &str = "ocara/workflow";

/// Profondeur maximale de l'arbre des imports.
const MAX_DEPTH: usize = 12;

pub fn workflow(ws: &Workspace, path: &Path) -> Value {
    let text = ws.text(path).unwrap_or_default();
    let root = ws.project_root(path);
    let mut seen = HashSet::from([path.to_path_buf()]);
    json!({
        "file": path,
        "imports": imports_of(ws, path, &root, &mut seen, 0),
        "structure": super::workflow_code::structure(ws, path, &text),
        "calls": super::workflow_code::calls(ws, path),
        "variants": variants(ws, path, &text),
    })
}

pub fn node(label: impl Into<String>, description: impl Into<String>, kind: &str, file: Option<&Path>, line: usize, children: Vec<Value>) -> Value {
    json!({ "label": label.into(), "description": description.into(), "kind": kind, "file": file, "line": line, "children": children })
}

/// Imports de `file` (builtins, fichiers du projet récursivement, runtimes).
fn imports_of(ws: &Workspace, file: &Path, root: &Path, seen: &mut HashSet<PathBuf>, depth: usize) -> Vec<Value> {
    let Some(program) = ws.text(file).and_then(|t| project::parse(&t)) else { return Vec::new() };
    let dir = file.parent().unwrap_or(root).to_path_buf();
    let mut out = Vec::new();
    for imp in &program.imports {
        let written = imp.path.join(".");
        if imp.path.first().is_some_and(|p| p == "ocara") {
            out.push(node(written, "builtin", "builtin", None, 0, Vec::new()));
            continue;
        }
        let (label, target) = match &imp.file_path {
            Some(fp) => (format!("{} from \"{}\"", written, fp), resolve_import_path(fp, &dir, &program.namespace, root)),
            None => (written, resolve_import_path(&imp.path.join("/"), root, &program.namespace, root)),
        };
        out.push(file_node(ws, label, "import", &target, root, seen, depth));
    }
    for rt in &program.runtime_imports {
        let kind = rt.kind.map(|k| format!(" is {}", k.as_str())).unwrap_or_default();
        let label = format!("runtime {}{}", rt.path.join("."), kind);
        match resolve_runtime_file(root, &rt.path) {
            Some(target) => out.push(file_node(ws, label, "runtime", &target, root, seen, depth)),
            None => out.push(node(label, "introuvable", "missing", None, 0, Vec::new())),
        }
    }
    out
}

fn file_node(ws: &Workspace, label: String, kind: &str, target: &Path, root: &Path, seen: &mut HashSet<PathBuf>, depth: usize) -> Value {
    let Ok(target) = target.canonicalize() else {
        return node(label, "introuvable", "missing", None, 0, Vec::new());
    };
    let shown = target.strip_prefix(root).unwrap_or(&target).display().to_string();
    if !seen.insert(target.clone()) {
        return node(label, format!("{} — déjà listé", shown), kind, Some(&target), 1, Vec::new());
    }
    let children = if depth < MAX_DEPTH { imports_of(ws, &target, root, seen, depth + 1) } else { Vec::new() };
    node(label, shown, kind, Some(&target), 1, children)
}

/// Variantes `when` du script et leur statut pour la configuration de
/// l'éditeur.
fn variants(ws: &Workspace, path: &Path, text: &str) -> Vec<Value> {
    let Some(analysis) = ws.analysis(path) else { return Vec::new() };
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<(usize, Value)> = analysis.variants.iter()
        .filter(|v| v.file.canonicalize().unwrap_or_else(|_| v.file.clone()) == path)
        .map(|v| {
            let first = lines.get(v.start_line.saturating_sub(1)).map(|l| l.trim()).unwrap_or("");
            let label = if first.starts_with("when") { first.to_string() } else { format!("sinon : {}", first) };
            let next = lines.get(v.start_line).map(|l| l.trim()).filter(|_| first.starts_with("when")).unwrap_or("");
            let (state, kind) = if v.retained { ("retenue", "active") } else { ("écartée", "inactive") };
            let description = if next.is_empty() { state.to_string() } else { format!("{} — {}", state, next) };
            (v.start_line, node(label, description, kind, Some(path), v.start_line, Vec::new()))
        })
        .collect();
    out.sort_by_key(|(line, _)| *line);
    out.into_iter().map(|(_, v)| v).collect()
}
