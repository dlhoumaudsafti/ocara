//! Références et CodeLens (implémentations, overrides, références) sur
//! l'index de l'espace de travail (`project`).

use std::path::Path;

use lsp_types::{CodeLens, Command, Location, Position};
use serde_json::json;

use super::decls;
use super::features::{parse_document, reference_at, to_url};
use super::position::{contains, name_range};
use super::project::{decl_key, start_of, DeclKey, ProjectIndex, TypeDecl};
use super::workspace::Workspace;
use crate::parsing::ast::{ClassMember, Program};
use crate::parsing::token::Span;
use crate::sema::index::Target;

pub fn references(ws: &Workspace, index: &ProjectIndex, path: &Path, pos: &Position, include_declaration: bool) -> Vec<Location> {
    let Some((key, declaration)) = key_at(ws, path, pos) else { return Vec::new() };
    let mut out = index.references(&key);
    if include_declaration {
        if let Some(d) = declaration {
            out.insert(0, d);
        }
    }
    out
}

/// Déclaration désignée au curseur (nom utilisé ou nom déclaré) et son emplacement.
fn key_at(ws: &Workspace, path: &Path, pos: &Position) -> Option<(DeclKey, Option<Location>)> {
    let analysis = ws.analysis(path)?;
    if let Some((r, _)) = reference_at(ws, analysis, path, pos) {
        let program = &analysis.checked.as_ref()?.program;
        let (span, name) = match &r.target {
            Target::Local { decl } => (decl.clone(), r.name.clone()),
            target => {
                let d = decls::find(program, target)?;
                (d.span, d.name)
            }
        };
        let key = decl_key(path, &span);
        let location = ws.text(&key.0).map(|t| Location::new(to_url(&key.0), name_range(&t, &span, &name)));
        return Some((key, location));
    }
    let text = ws.text(path)?;
    let own = parse_document(ws, path)?;
    declarations(&own).into_iter()
        .map(|(name, span)| (name_range(&text, &span, &name), span))
        .find(|(range, _)| contains(range, pos))
        .map(|(range, span)| (decl_key(path, &span), Some(Location::new(to_url(path), range))))
}

/// Noms déclarés par un fichier, avec leur span.
fn declarations(program: &Program) -> Vec<(String, Span)> {
    let mut out = Vec::new();
    let members = |members: &[ClassMember], out: &mut Vec<(String, Span)>| {
        for m in members {
            match m {
                ClassMember::Method { decl, .. } => out.push((decl.name.clone(), decl.span.clone())),
                ClassMember::Field { name, span, .. } | ClassMember::Const { name, span, .. } => out.push((name.clone(), span.clone())),
                ClassMember::Constructor { .. } => {}
            }
        }
    };
    for c in &program.classes { out.push((c.name.clone(), c.span.clone())); members(&c.members, &mut out); }
    for g in &program.generics { out.push((g.name.clone(), g.span.clone())); members(&g.members, &mut out); }
    for m in &program.modules { out.push((m.name.clone(), m.span.clone())); members(&m.members, &mut out); }
    for i in &program.interfaces {
        out.push((i.name.clone(), i.span.clone()));
        out.extend(i.methods.iter().map(|m| (m.name.clone(), m.span.clone())));
    }
    out.extend(program.enums.iter().map(|e| (e.name.clone(), e.span.clone())));
    out.extend(program.functions.iter().map(|f| (f.name.clone(), f.span.clone())));
    out.extend(program.consts.iter().map(|c| (c.name.clone(), c.span.clone())));
    out
}

// ── CodeLens ────────────────────────────────────────────────────────────────

pub fn code_lenses(ws: &Workspace, index: &ProjectIndex, path: &Path) -> Vec<CodeLens> {
    let (Some(text), Some(own)) = (ws.text(path), parse_document(ws, path)) else { return Vec::new() };
    let types = index.types_for(path);
    let at = |span: &Span, name: &str| Location::new(to_url(path), name_range(&text, span, name));
    let refs = |span: &Span| index.references(&decl_key(path, span));
    let mut lenses = Vec::new();

    for c in own.classes.iter().map(|c| (&c.name, &c.span, &c.members, c.is_struct))
        .chain(own.generics.iter().map(|g| (&g.name, &g.span, &g.members, false)))
    {
        let (name, span, members, is_struct) = c;
        let here = at(span, name);
        let subclasses = all(&types, |t| descendants(name, t));
        lenses.push(lens(&here, "implémentation", subclasses.iter().map(|t| t.location.clone()).collect()));
        let methods = method_names(members);
        if !is_struct {
            lenses.push(lens(&here, "override", overrides(&subclasses, &methods)));
        }
        lenses.push(lens(&here, "référence", refs(span)));
        for (m, m_span) in &methods {
            let loc = at(m_span, m);
            lenses.push(lens(&loc, "override", overrides(&subclasses, &[(m.clone(), m_span.clone())])));
            lenses.push(lens(&loc, "référence", refs(m_span)));
        }
    }

    for i in &own.interfaces {
        let here = at(&i.span, &i.name);
        let implementers = all(&types, |t| with_descendants(t.iter().filter(|c| c.implements.contains(&i.name)).cloned().collect(), t));
        let methods: Vec<(String, Span)> = i.methods.iter().map(|m| (m.name.clone(), m.span.clone())).collect();
        lenses.push(lens(&here, "implémentation", implementers.iter().map(|t| t.location.clone()).collect()));
        lenses.push(lens(&here, "override", overrides(&implementers, &methods)));
        lenses.push(lens(&here, "référence", refs(&i.span)));
        for (m, m_span) in &methods {
            let loc = at(m_span, m);
            lenses.push(lens(&loc, "implémentation", overrides(&implementers, &[(m.clone(), m_span.clone())])));
            lenses.push(lens(&loc, "référence", refs(m_span)));
        }
    }

    for module in &own.modules {
        let here = at(&module.span, &module.name);
        let users = all(&types, |t| with_descendants(t.iter().filter(|c| c.modules.contains(&module.name)).cloned().collect(), t));
        let methods = method_names(&module.members);
        lenses.push(lens(&here, "implémentation", users.iter().map(|t| t.location.clone()).collect()));
        lenses.push(lens(&here, "override", overrides(&users, &methods)));
        for (m, m_span) in &methods {
            let loc = at(m_span, m);
            lenses.push(lens(&loc, "override", overrides(&users, &[(m.clone(), m_span.clone())])));
            lenses.push(lens(&loc, "référence", refs(m_span)));
        }
    }

    for e in &own.enums {
        lenses.push(lens(&at(&e.span, &e.name), "référence", refs(&e.span)));
    }
    for f in &own.functions {
        lenses.push(lens(&at(&f.span, &f.name), "référence", refs(&f.span)));
    }
    lenses
}

fn lens(at: &Location, noun: &str, locations: Vec<Location>) -> CodeLens {
    let n = locations.len();
    let title = format!("{} {}{}", n, noun, if n > 1 { "s" } else { "" });
    let command = if n == 0 {
        Command { title, command: String::new(), arguments: None }
    } else {
        Command {
            title,
            command: "ocara.showReferences".into(),
            arguments: Some(vec![json!(at.uri), json!(at.range.start), json!(locations)]),
        }
    };
    CodeLens { range: start_of(at), command: Some(command), data: None }
}

fn method_names(members: &[ClassMember]) -> Vec<(String, Span)> {
    members.iter().filter_map(|m| match m {
        ClassMember::Method { decl, .. } => Some((decl.name.clone(), decl.span.clone())),
        _ => None,
    }).collect()
}

/// Méthodes des types `among` qui portent le nom d'une des `methods`.
fn overrides(among: &[TypeDecl], methods: &[(String, Span)]) -> Vec<Location> {
    among.iter()
        .flat_map(|t| t.methods.iter().filter(|(n, _)| methods.iter().any(|(m, _)| m == n)).map(|(_, loc)| loc.clone()))
        .collect()
}

/// Résultat de `f` pour chaque entrée, sans doublon.
fn all(entries: &[Vec<TypeDecl>], f: impl Fn(&[TypeDecl]) -> Vec<TypeDecl>) -> Vec<TypeDecl> {
    let mut out: Vec<TypeDecl> = Vec::new();
    for types in entries {
        for t in f(types) {
            if !out.iter().any(|o| o.key == t.key) {
                out.push(t);
            }
        }
    }
    out
}

/// Classes descendant de `name` via `extends` (transitif, anti-cycle).
fn descendants(name: &str, types: &[TypeDecl]) -> Vec<TypeDecl> {
    let mut result: Vec<TypeDecl> = Vec::new();
    let mut queue = vec![name.to_string()];
    while let Some(current) = queue.pop() {
        for t in types {
            if t.extends.as_deref() == Some(current.as_str()) && t.name != name && !result.iter().any(|r| r.key == t.key) {
                result.push(t.clone());
                queue.push(t.name.clone());
            }
        }
    }
    result
}

fn with_descendants(roots: Vec<TypeDecl>, types: &[TypeDecl]) -> Vec<TypeDecl> {
    let mut all = roots.clone();
    for root in &roots {
        for d in descendants(&root.name, types) {
            if !all.iter().any(|a| a.key == d.key) {
                all.push(d);
            }
        }
    }
    all
}
