//! Requête `ocara/workflow` (encart « Workflow » de l'extension), en
//! partant du script affiché : arbre des imports (runtimes compris),
//! structure (types, membres, variables), graphe d'appels et variantes
//! `when`. Chaque nœud : `label`, `description`, `kind` (icône), `file` et
//! `line` (1-based) pour l'ouvrir, `children`, et `inactive` quand il est
//! déclaré dans une variante `when` écartée pour la configuration active.

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
    let inactive = Inactive::of(ws, path);
    let mut structure = Value::from(super::workflow_code::structure(ws, path, &text));
    let mut calls = super::workflow_code::calls(ws, path);
    inactive.mark(&mut structure);
    if let Some(nodes) = calls.get_mut("nodes") {
        inactive.mark(nodes);
    }
    json!({
        "file": path,
        "imports": imports_of(ws, path, &root, &mut seen, &inactive, 0),
        "structure": structure,
        "calls": calls,
        "variants": variants(ws, path, &text),
    })
}

/// Lignes des variantes `when` écartées pour la configuration active.
struct Inactive(Vec<(PathBuf, usize, usize)>);

impl Inactive {
    fn of(ws: &Workspace, path: &Path) -> Self {
        let ranges = ws.analysis(path).map(|a| a.variants.iter()
            .filter(|v| !v.retained)
            .map(|v| (v.file.canonicalize().unwrap_or_else(|_| v.file.clone()), v.start_line, v.end_line))
            .collect()).unwrap_or_default();
        Self(ranges)
    }

    fn contains(&self, file: &Path, line: usize) -> bool {
        self.0.iter().any(|(f, start, end)| f == file && (*start..=*end).contains(&line))
    }

    /// Nœuds (et leurs enfants) situés dans une variante écartée.
    fn mark(&self, value: &mut Value) {
        match value {
            Value::Array(items) => items.iter_mut().for_each(|v| self.mark(v)),
            Value::Object(map) => {
                let file = map.get("file").and_then(Value::as_str).map(PathBuf::from);
                let line = map.get("line").and_then(Value::as_u64).unwrap_or(0) as usize;
                if file.is_some_and(|f| self.contains(&f, line)) {
                    map.insert("inactive".into(), Value::Bool(true));
                }
                map.values_mut().for_each(|v| self.mark(v));
            }
            _ => {}
        }
    }
}

pub fn node(label: impl Into<String>, description: impl Into<String>, kind: &str, file: Option<&Path>, line: usize, children: Vec<Value>) -> Value {
    json!({ "label": label.into(), "description": description.into(), "kind": kind, "file": file, "line": line, "children": children })
}

/// Imports de `file` (builtins, fichiers du projet récursivement, runtimes).
fn imports_of(ws: &Workspace, file: &Path, root: &Path, seen: &mut HashSet<PathBuf>, inactive: &Inactive, depth: usize) -> Vec<Value> {
    let Some(program) = ws.text(file).and_then(|t| project::parse(&t)) else { return Vec::new() };
    let dir = file.parent().unwrap_or(root).to_path_buf();
    let mut out = Vec::new();
    for imp in &program.imports {
        let written = imp.path.join(".");
        let mut item = if imp.path.first().is_some_and(|p| p == "ocara") {
            node(written, "builtin", "builtin", None, 0, Vec::new())
        } else {
            let (label, target) = match &imp.file_path {
                Some(fp) => (format!("{} from \"{}\"", written, fp), resolve_import_path(fp, &dir, &program.namespace, root)),
                None => (written, resolve_import_path(&imp.path.join("/"), root, &program.namespace, root)),
            };
            file_node(ws, label, "import", &target, root, seen, inactive, depth)
        };
        // Import écarté par sa clause `when` : estompé dans l'encart.
        if inactive.contains(file, imp.span.line) {
            item["inactive"] = Value::Bool(true);
        }
        out.push(item);
    }
    for rt in &program.runtime_imports {
        let kind = rt.kind.map(|k| format!(" is {}", k.as_str())).unwrap_or_default();
        let label = format!("runtime {}{}", rt.path.join("."), kind);
        let mut item = match resolve_runtime_file(root, &rt.path) {
            Some(target) => file_node(ws, label, "runtime", &target, root, seen, inactive, depth),
            None => node(label, "introuvable", "missing", None, 0, Vec::new()),
        };
        if inactive.contains(file, rt.span.line) {
            item["inactive"] = Value::Bool(true);
        }
        out.push(item);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn file_node(ws: &Workspace, label: String, kind: &str, target: &Path, root: &Path, seen: &mut HashSet<PathBuf>, inactive: &Inactive, depth: usize) -> Value {
    let Ok(target) = target.canonicalize() else {
        return node(label, "introuvable", "missing", None, 0, Vec::new());
    };
    let shown = target.strip_prefix(root).unwrap_or(&target).display().to_string();
    if !seen.insert(target.clone()) {
        return node(label, format!("{} — déjà listé", shown), kind, Some(&target), 1, Vec::new());
    }
    let children = if depth < MAX_DEPTH { imports_of(ws, &target, root, seen, inactive, depth + 1) } else { Vec::new() };
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
