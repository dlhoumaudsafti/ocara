//! Graphe d'appels de l'encart Workflow : fonctions, méthodes (classes,
//! structs, generics, modules, interfaces) et blocs runtime du programme
//! analysé ; chaque référence à une fonction ou une méthode est rattachée
//! au corps qui la contient. `roots` : ce que déclare le script affiché (et,
//! pour un `main.oc`, ses blocs runtime, fichiers runtime compris).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::decls::{self, display_type};
use super::features::span_path;
use super::workflow_code::{body_ends, end_of};
use super::workspace::Workspace;
use crate::core::runtime_expand::get_stmt_span;
use crate::parsing::ast::{ClassMember, FuncDecl, Param, Program, Type};
use crate::sema::index::Target;

struct Callable {
    /// Classe, struct, interface… ; `None` pour une fonction libre.
    owner: Option<String>,
    /// class, struct, generic, module, interface, function, runtime.
    kind: &'static str,
    name: String,
    params: String,
    ret: String,
    is_static: bool,
    file: PathBuf,
    line: usize,
    end: usize,
}

impl Callable {
    fn key(&self) -> String {
        format!("{}:{}", self.file.display(), self.line)
    }

    fn label(&self) -> String {
        match &self.owner {
            Some(owner) if self.kind != "runtime" => format!("{}{}{}", owner, if self.is_static { "::" } else { "." }, self.name),
            _ => self.name.clone(),
        }
    }
}

pub fn calls(ws: &Workspace, path: &Path) -> Value {
    let Some(checked) = ws.analysis(path).and_then(|a| a.checked.as_ref()) else { return json!({ "roots": [], "nodes": {} }) };
    let entry = ws.entry_for(path, &ws.project_root(path));
    let program = &checked.program;
    let mut ends: HashMap<PathBuf, BTreeMap<usize, usize>> = HashMap::new();
    let mut end_in = |file: &Path, line: usize| -> usize {
        let file_ends = ends.entry(file.to_path_buf()).or_insert_with(|| ws.text(file).map(|t| body_ends(&t)).unwrap_or_default());
        end_of(file_ends, line)
    };
    let mut callables = declared(program, &entry, &mut end_in);
    callables.extend(runtime_blocks(program, &entry, &mut end_in));

    let mut callees: HashMap<String, Vec<String>> = HashMap::new();
    for r in &checked.references {
        if !matches!(r.target, Target::Function(_) | Target::Method { .. }) {
            continue;
        }
        let Some(decl) = decls::find(program, &r.target) else { continue };
        let target = span_path(&decl.span, &entry);
        let Some(callee) = callables.iter().find(|c| c.file == target && c.line == decl.span.line) else { continue };
        let file = span_path(&r.span, &entry);
        let Some(owner) = callables.iter()
            .filter(|c| c.file == file && (c.line..=c.end).contains(&r.span.line))
            .min_by_key(|c| c.end - c.line) else { continue };
        let list = callees.entry(owner.key()).or_default();
        if !list.contains(&callee.key()) {
            list.push(callee.key());
        }
    }

    let mut nodes = serde_json::Map::new();
    for c in &callables {
        nodes.insert(c.key(), json!({
            "label": c.label(), "owner": c.owner, "kind": c.kind, "name": c.name, "params": c.params, "ret": c.ret,
            "static": c.is_static, "file": c.file, "line": c.line, "callees": callees.get(&c.key()).cloned().unwrap_or_default(),
        }));
    }
    let shown = |c: &&Callable| c.file == path || (c.kind == "runtime" && path == entry);
    let roots: Vec<String> = callables.iter().filter(shown).map(Callable::key).collect();
    json!({ "roots": roots, "nodes": nodes })
}

fn signature(params: &[Param], ret: &Type) -> (String, String) {
    (params.iter().map(decls::param_label).collect::<Vec<_>>().join(", "), display_type(ret))
}

fn callable(owner: Option<&str>, kind: &'static str, f: &FuncDecl, is_static: bool, entry: &Path, end_in: &mut impl FnMut(&Path, usize) -> usize) -> Callable {
    let file = span_path(&f.span, entry);
    let (params, ret) = signature(&f.params, &f.ret_ty);
    let end = end_in(&file, f.span.line);
    Callable { owner: owner.map(str::to_string), kind, name: f.name.clone(), params, ret, is_static, file, line: f.span.line, end }
}

/// Fonctions libres, méthodes des types et signatures d'interface.
fn declared(program: &Program, entry: &Path, end_in: &mut impl FnMut(&Path, usize) -> usize) -> Vec<Callable> {
    let mut out: Vec<Callable> = program.functions.iter().map(|f| callable(None, "function", f, false, entry, end_in)).collect();
    let owners = program.classes.iter().map(|c| (&c.name, &c.members, if c.is_struct { "struct" } else { "class" }))
        .chain(program.generics.iter().map(|g| (&g.name, &g.members, "generic")))
        .chain(program.modules.iter().map(|m| (&m.name, &m.members, "module")));
    for (owner, members, kind) in owners {
        for m in members {
            match m {
                ClassMember::Method { decl, is_static, .. } => out.push(callable(Some(owner), kind, decl, *is_static, entry, end_in)),
                ClassMember::Constructor { params, span, .. } => {
                    let file = span_path(span, entry);
                    let (params, _) = signature(params, &Type::Void);
                    let end = end_in(&file, span.line);
                    out.push(Callable { owner: Some(owner.clone()), kind, name: "init".into(), params, ret: String::new(), is_static: false, file, line: span.line, end });
                }
                _ => {}
            }
        }
    }
    for i in &program.interfaces {
        for m in &i.methods {
            let (params, ret) = signature(&m.params, &m.ret_ty);
            let file = span_path(&m.span, entry);
            out.push(Callable { owner: Some(i.name.clone()), kind: "interface", name: m.name.clone(), params, ret, is_static: m.is_static, file, line: m.span.line, end: m.span.line });
        }
    }
    out
}

/// Blocs runtime : un fichier runtime entier (`runtime core.main is main`),
/// ou le bloc écrit dans le fichier d'entrée (`main { … }`).
fn runtime_blocks(program: &Program, entry: &Path, end_in: &mut impl FnMut(&Path, usize) -> usize) -> Vec<Callable> {
    let mut out: Vec<Callable> = Vec::new();
    for block in &program.runtime_blocks {
        let kind = block.kind.as_str();
        let mut files: Vec<PathBuf> = Vec::new();
        for stmt in &block.statements {
            let file = span_path(get_stmt_span(stmt), entry);
            if !files.contains(&file) {
                files.push(file);
            }
        }
        for file in files {
            let (line, end) = if file == entry { (block.span.line, end_in(&file, block.span.line)) } else { (1, usize::MAX) };
            if out.iter().any(|c| c.file == file && c.line == line) {
                continue;
            }
            let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            out.push(Callable { owner: Some(format!("runtime {}", kind)), kind: "runtime", name, params: String::new(), ret: String::new(), is_static: false, file, line, end });
        }
    }
    out
}
