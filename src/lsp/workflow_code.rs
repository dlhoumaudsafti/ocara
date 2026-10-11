//! Encart « Workflow » : structure du script (types, membres, variables
//! locales typées par la sema). Graphe d'appels : workflow_calls.rs.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use super::decls::{self, display_type, function_signature};
use super::features::span_path;
use super::project;
use super::workflow::node;
use super::workspace::Workspace;
use crate::parsing::ast::{ClassMember, FuncDecl};
use crate::parsing::lexer::Lexer;
use crate::parsing::token::TokenKind;
use crate::sema::index::Target;

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
pub(super) fn body_ends(text: &str) -> BTreeMap<usize, usize> {
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
pub(super) fn end_of(ends: &BTreeMap<usize, usize>, line: usize) -> usize {
    ends.range(line..).next().map(|(_, end)| *end).unwrap_or(line)
}
