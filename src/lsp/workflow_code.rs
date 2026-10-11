//! Encart « Workflow » : structure du script (types, membres, variables
//! locales typées par la sema) et graphe d'appels (fonctions et méthodes
//! appelées par chaque fonction, sur tout le programme analysé).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::decls::{self, display_type, function_signature};
use super::features::span_path;
use super::project;
use super::workflow::node;
use super::workspace::Workspace;
use crate::parsing::ast::{ClassMember, FuncDecl, Program};
use crate::parsing::lexer::Lexer;
use crate::parsing::token::{Span, TokenKind};
use crate::sema::index::Target;

/// Fonction ou méthode : position de sa déclaration et lignes de son corps.
struct Callable {
    label: String,
    file: PathBuf,
    line: usize,
    end: usize,
}

pub fn structure(ws: &Workspace, path: &Path, text: &str) -> Vec<Value> {
    let Some(program) = project::parse(text) else { return Vec::new() };
    let ends = body_ends(text);
    let locals = locals_by_line(ws, path);
    let mut fn_node = |label: String, kind: &str, f: &FuncDecl| {
        let end = end_of(&ends, f.span.line);
        let vars: Vec<Value> = locals.iter()
            .filter(|(line, ..)| (f.span.line..=end).contains(line))
            .map(|(line, name, ty)| node(name.clone(), ty.clone(), "variable", Some(path), *line, Vec::new()))
            .collect();
        node(label, function_signature("", f).trim_start_matches(&f.name).to_string(), kind, Some(path), f.span.line, vars)
    };
    let mut out: Vec<(usize, Value)> = Vec::new();
    for c in &program.classes {
        let members = members(&c.members, path, &mut fn_node);
        let kind = if c.is_struct { "struct" } else { "class" };
        let description = c.extends.as_ref().map(|e| format!("{} extends {}", kind, e)).unwrap_or_else(|| kind.to_string());
        out.push((c.span.line, node(c.name.clone(), description, kind, Some(path), c.span.line, members)));
    }
    for g in &program.generics {
        let params = g.type_params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ");
        out.push((g.span.line, node(format!("{}<{}>", g.name, params), "generic", "class", Some(path), g.span.line, members(&g.members, path, &mut fn_node))));
    }
    for m in &program.modules {
        out.push((m.span.line, node(m.name.clone(), "module", "module", Some(path), m.span.line, members(&m.members, path, &mut fn_node))));
    }
    for i in &program.interfaces {
        let methods = i.methods.iter()
            .map(|m| node(m.name.clone(), format!("({}): {}", m.params.iter().map(decls::param_label).collect::<Vec<_>>().join(", "), display_type(&m.ret_ty)), "method", Some(path), m.span.line, Vec::new()))
            .collect();
        out.push((i.span.line, node(i.name.clone(), "interface", "interface", Some(path), i.span.line, methods)));
    }
    for e in &program.enums {
        let variants = e.variants.iter().map(|v| node(v.name.clone(), "", "enum-member", Some(path), v.span.line, Vec::new())).collect();
        out.push((e.span.line, node(e.name.clone(), "enum", "enum", Some(path), e.span.line, variants)));
    }
    for c in &program.consts {
        out.push((c.span.line, node(c.name.clone(), display_type(&c.ty), "constant", Some(path), c.span.line, Vec::new())));
    }
    for f in &program.functions {
        out.push((f.span.line, fn_node(f.name.clone(), "function", f)));
    }
    for b in &program.runtime_blocks {
        out.push((b.span.line, node(format!("{} {{ }}", b.kind.as_str()), "bloc runtime", "runtime", Some(path), b.span.line, Vec::new())));
    }
    out.sort_by_key(|(line, _)| *line);
    out.into_iter().map(|(_, v)| v).collect()
}

fn members(members: &[ClassMember], path: &Path, fn_node: &mut impl FnMut(String, &str, &FuncDecl) -> Value) -> Vec<Value> {
    members.iter().filter_map(|m| match m {
        ClassMember::Field { name, ty, span, .. } => Some(node(name.clone(), display_type(ty), "property", Some(path), span.line, Vec::new())),
        ClassMember::Const { name, ty, span, .. } => Some(node(name.clone(), display_type(ty), "constant", Some(path), span.line, Vec::new())),
        ClassMember::Method { decl, is_static, .. } => Some(fn_node(decl.name.clone(), if *is_static { "static-method" } else { "method" }, decl)),
        ClassMember::Constructor { span, .. } => Some(node("init", "constructeur", "constructor", Some(path), span.line, Vec::new())),
    }).collect()
}

/// Variables locales (paramètres compris) déclarées dans `path`, typées par
/// la sema : (ligne, nom, type).
fn locals_by_line(ws: &Workspace, path: &Path) -> Vec<(usize, String, String)> {
    let Some(checked) = ws.analysis(path).and_then(|a| a.checked.as_ref()) else { return Vec::new() };
    let entry = ws.entry_for(path, &ws.project_root(path));
    let mut out: Vec<(usize, String, String)> = Vec::new();
    for r in &checked.references {
        let Target::Local { decl } = &r.target else { continue };
        if decl.line != r.span.line || decl.col != r.span.col || span_path(&r.span, &entry) != path {
            continue;
        }
        if !out.iter().any(|(l, n, _)| *l == decl.line && *n == r.name) {
            out.push((decl.line, r.name.clone(), display_type(&r.ty)));
        }
    }
    out.sort();
    out
}

/// Ligne de début d'un bloc `{ … }` → ligne de sa fermeture, pour chaque
/// première accolade ouvrante d'une ligne.
fn body_ends(text: &str) -> BTreeMap<usize, usize> {
    let Ok(tokens) = Lexer::new(text).tokenize() else { return BTreeMap::new() };
    let mut stack: Vec<usize> = Vec::new();
    let mut ends: BTreeMap<usize, usize> = BTreeMap::new();
    for tok in &tokens {
        match tok.kind {
            TokenKind::LBrace => stack.push(tok.span.line),
            TokenKind::RBrace => {
                if let Some(start) = stack.pop() {
                    ends.entry(start).or_insert(tok.span.line);
                }
            }
            _ => {}
        }
    }
    ends
}

/// Fin du corps d'une déclaration commencée ligne `line` : premier bloc
/// ouvert à partir de cette ligne (paramètres sur plusieurs lignes).
fn end_of(ends: &BTreeMap<usize, usize>, line: usize) -> usize {
    ends.range(line..).next().map(|(_, end)| *end).unwrap_or(line)
}

/// Graphe d'appels : `roots` (fonctions et méthodes du script, et le code
/// hors fonction) et `nodes` (clé → libellé, position, appelés).
pub fn calls(ws: &Workspace, path: &Path) -> Value {
    let Some(checked) = ws.analysis(path).and_then(|a| a.checked.as_ref()) else { return json!({ "roots": [], "nodes": {} }) };
    let entry = ws.entry_for(path, &ws.project_root(path));
    let callables = callables(ws, &checked.program, &entry);
    let key = |c: &Callable| format!("{}:{}", c.file.display(), c.line);
    let outside = format!("{}:0", path.display());
    let mut callees: HashMap<String, Vec<String>> = HashMap::new();
    for r in &checked.references {
        if !matches!(r.target, Target::Function(_) | Target::Method { .. }) {
            continue;
        }
        let Some(decl) = decls::find(&checked.program, &r.target) else { continue };
        let target = span_path(&decl.span, &entry);
        let Some(callee) = callables.iter().find(|c| c.file == target && c.line == decl.span.line) else { continue };
        let file = span_path(&r.span, &entry);
        let owner = callables.iter()
            .filter(|c| c.file == file && (c.line..=c.end).contains(&r.span.line))
            .min_by_key(|c| c.end - c.line)
            .map(&key)
            .or_else(|| (file == path).then(|| outside.clone()));
        let Some(owner) = owner else { continue };
        let list = callees.entry(owner).or_default();
        if !list.contains(&key(callee)) {
            list.push(key(callee));
        }
    }
    let mut nodes = serde_json::Map::new();
    for c in &callables {
        nodes.insert(key(c), json!({ "label": c.label, "file": c.file, "line": c.line, "callees": callees.get(&key(c)).cloned().unwrap_or_default() }));
    }
    let mut roots: Vec<String> = callables.iter().filter(|c| c.file == path).map(&key).collect();
    if let Some(list) = callees.get(&outside) {
        nodes.insert(outside.clone(), json!({ "label": "code hors fonction (blocs runtime…)", "file": path, "line": 1, "callees": list }));
        roots.insert(0, outside);
    }
    json!({ "roots": roots, "nodes": nodes })
}

/// Fonctions et méthodes du programme analysé (fichiers importés compris).
fn callables(ws: &Workspace, program: &Program, entry: &Path) -> Vec<Callable> {
    let mut decls: Vec<(String, Span)> = program.functions.iter().map(|f| (f.name.clone(), f.span.clone())).collect();
    let owners = program.classes.iter().map(|c| (&c.name, &c.members))
        .chain(program.generics.iter().map(|g| (&g.name, &g.members)))
        .chain(program.modules.iter().map(|m| (&m.name, &m.members)));
    for (owner, members) in owners {
        for m in members {
            if let ClassMember::Method { decl, is_static, .. } = m {
                decls.push((format!("{}{}{}", owner, if *is_static { "::" } else { "." }, decl.name), decl.span.clone()));
            }
        }
    }
    let mut ends: HashMap<PathBuf, BTreeMap<usize, usize>> = HashMap::new();
    decls.into_iter().map(|(label, span)| {
        let file = span_path(&span, entry);
        let file_ends = ends.entry(file.clone()).or_insert_with(|| ws.text(&file).map(|t| body_ends(&t)).unwrap_or_default());
        let end = end_of(file_ends, span.line);
        Callable { label, file, line: span.line, end }
    }).collect()
}
