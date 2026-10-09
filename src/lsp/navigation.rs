//! Références et CodeLens (implémentations, overrides, références) sur
//! l'index de l'espace de travail (`project`).

use std::path::Path;

use std::collections::HashMap;

use lsp_types::{
    CodeLens, Command, DocumentChangeOperation, DocumentChanges, Location, OneOf, OptionalVersionedTextDocumentIdentifier,
    Position, RenameFile, ResourceOp, TextDocumentEdit, TextEdit, Url, WorkspaceEdit,
};
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

// ── Renommage ───────────────────────────────────────────────────────────────

/// Renomme la déclaration au curseur et toutes ses références ; une méthode
/// redéfinie l'est avec toute sa chaîne (parents, sous-classes, interfaces).
/// Refusé pour un builtin ou un nom invalide.
pub fn rename(ws: &Workspace, index: &ProjectIndex, path: &Path, pos: &Position, new_name: &str) -> Result<WorkspaceEdit, String> {
    if !is_identifier(new_name) {
        return Err(format!("« {} » n'est pas un nom valide", new_name));
    }
    let (key, declaration) = key_at(ws, path, pos).ok_or("rien à renommer ici")?;
    let declaration = declaration.ok_or("déclaration introuvable (builtin ?)")?;
    let old_name = name_at(ws, &declaration).ok_or("déclaration introuvable")?;
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    let mut add = |loc: Location| {
        let edits = changes.entry(loc.uri).or_default();
        if !edits.iter().any(|e| e.range == loc.range) {
            edits.push(TextEdit::new(loc.range, new_name.to_string()));
        }
    };
    // Méthode d'une chaîne de redéfinition : toutes les méthodes liées
    // (parents, sous-classes, interfaces) sont renommées ensemble.
    let family = method_family(index, path, &declaration);
    let mut locations = Vec::new();
    for (k, loc) in family.iter().cloned().chain(std::iter::once((key.clone(), declaration.clone()))) {
        locations.extend(index.references(&k));
        locations.push(loc);
    }
    let top_level = is_top_level(ws, &key.0, &declaration, &old_name);
    // Fichier qui porte le nom de la déclaration : renommé avec elle (un
    // import par namespace désigne le fichier).
    let renamed_file = top_level
        .then(|| key.0.clone())
        .filter(|f| f.file_stem().is_some_and(|stem| *stem == *old_name))
        .map(|f| (f.clone(), f.with_file_name(format!("{}.oc", new_name))));
    if top_level {
        locations.extend(import_locations(ws, index, &old_name, renamed_file.is_some()));
    }
    for loc in locations {
        add(loc);
    }

    let Some((from, to)) = renamed_file else {
        return Ok(WorkspaceEdit { changes: Some(changes), ..Default::default() });
    };
    if to.exists() {
        return Err(format!("le fichier {} existe déjà", to.display()));
    }
    let mut operations: Vec<DocumentChangeOperation> = changes.into_iter().map(|(uri, edits)| {
        DocumentChangeOperation::Edit(TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
            edits: edits.into_iter().map(OneOf::Left).collect(),
        })
    }).collect();
    operations.push(DocumentChangeOperation::Op(ResourceOp::Rename(RenameFile {
        old_uri: to_url(&from), new_uri: to_url(&to), options: None, annotation_id: None,
    })));
    Ok(WorkspaceEdit { document_changes: Some(DocumentChanges::Operations(operations)), ..Default::default() })
}

fn name_at(ws: &Workspace, loc: &Location) -> Option<String> {
    let path = loc.uri.to_file_path().ok()?;
    let text = ws.text(&path)?;
    let line = text.lines().nth(loc.range.start.line as usize)?;
    Some(line.chars().skip(loc.range.start.character as usize).take((loc.range.end.character - loc.range.start.character) as usize).collect())
}

/// Classe, interface, generic, module, enum ou fonction de premier niveau.
fn is_top_level(ws: &Workspace, file: &Path, declaration: &Location, name: &str) -> bool {
    let (Some(text), Some(own)) = (ws.text(file), ws.text(file).as_deref().and_then(super::project::parse)) else { return false };
    let spans = own.classes.iter().map(|c| (&c.name, &c.span))
        .chain(own.interfaces.iter().map(|i| (&i.name, &i.span)))
        .chain(own.generics.iter().map(|g| (&g.name, &g.span)))
        .chain(own.modules.iter().map(|m| (&m.name, &m.span)))
        .chain(own.enums.iter().map(|e| (&e.name, &e.span)))
        .chain(own.functions.iter().map(|f| (&f.name, &f.span)));
    spans.into_iter().any(|(n, span)| n == name && name_range(&text, span, n) == declaration.range)
}

/// Lignes `import a.b.Nom`, `import Nom from "…/Nom"` et `wiring a.b.Nom` de
/// l'espace de travail ; le chemin d'un fichier renommé est mis à jour aussi.
fn import_locations(ws: &Workspace, index: &ProjectIndex, name: &str, file_renamed: bool) -> Vec<Location> {
    let mut out = Vec::new();
    for file in index.files() {
        let Some(text) = ws.text(&file) else { continue };
        let Some(own) = super::project::parse(&text) else { continue };
        let mut on_line = |span: &Span, occurrences: usize| {
            let mut from = span.clone();
            for _ in 0..occurrences {
                let range = name_range(&text, &from, name);
                if range.start == range.end { break; }
                out.push(Location::new(to_url(&file), range));
                from.col = range.end.character as usize + 2;
            }
        };
        for imp in &own.imports {
            if imp.path.first().is_some_and(|p| p == "ocara") { continue; }
            match &imp.file_path {
                None if imp.path.last().is_some_and(|l| l == name) => on_line(&imp.span, 1),
                Some(path) if imp.path.first().is_some_and(|p| p == name) => {
                    let in_path = file_renamed && path.trim_end_matches(".oc").rsplit('/').next() == Some(name);
                    on_line(&imp.span, if in_path { 2 } else { 1 });
                }
                _ => {}
            }
        }
        for w in own.interfaces.iter().flat_map(|i| &i.wirings) {
            if w.path.last().is_some_and(|l| l == name) {
                on_line(&w.span, 1);
            }
        }
    }
    out
}

fn is_identifier(name: &str) -> bool {
    use crate::parsing::token::TokenKind;
    let Ok(tokens) = crate::parsing::lexer::Lexer::new(name).tokenize() else { return false };
    matches!(tokens.as_slice(), [t, _] if matches!(t.kind, TokenKind::Ident(_)))
}

/// Méthodes liées à la méthode déclarée en `declaration` par redéfinition :
/// ancêtres et interfaces qui la déclarent, puis leurs sous-classes et
/// classes d'implémentation qui la redéclarent (clé et emplacement du nom).
fn method_family(index: &ProjectIndex, path: &Path, declaration: &Location) -> Vec<(DeclKey, Location)> {
    let mut family: Vec<(DeclKey, Location)> = Vec::new();
    for types in index.types_for(path) {
        let Some((owner, method)) = types.iter().find_map(|t| {
            t.methods.iter().find(|(_, loc, _)| loc == declaration).map(|(m, _, _)| (t, m.clone()))
        }) else { continue };
        let declares = |t: &TypeDecl| t.methods.iter().any(|(m, _, _)| *m == method);
        let mut seeds: Vec<&TypeDecl> = vec![owner];
        seeds.extend(supertypes(owner, &types).into_iter().filter(|t| declares(t)));
        let mut members: Vec<&TypeDecl> = seeds.clone();
        for seed in &seeds {
            for t in subtypes(seed, &types) {
                if declares(t) && !members.iter().any(|m| m.key == t.key) {
                    members.push(t);
                }
            }
        }
        for t in members {
            for (m, loc, key) in &t.methods {
                if *m == method && !family.iter().any(|(k, _)| k == key) {
                    family.push((key.clone(), loc.clone()));
                }
            }
        }
    }
    family
}

/// Ancêtres (`extends`) et interfaces implémentées, transitivement.
fn supertypes<'a>(t: &TypeDecl, types: &'a [TypeDecl]) -> Vec<&'a TypeDecl> {
    let mut out: Vec<&TypeDecl> = Vec::new();
    let mut queue: Vec<String> = t.extends.iter().chain(&t.implements).cloned().collect();
    while let Some(name) = queue.pop() {
        for found in types.iter().filter(|x| x.name == name) {
            if !out.iter().any(|o| o.key == found.key) {
                out.push(found);
                queue.extend(found.extends.iter().chain(&found.implements).cloned());
            }
        }
    }
    out
}

/// Sous-classes et, pour une interface, classes d'implémentation et leurs
/// sous-classes, transitivement.
fn subtypes<'a>(t: &TypeDecl, types: &'a [TypeDecl]) -> Vec<&'a TypeDecl> {
    let mut out: Vec<&TypeDecl> = Vec::new();
    let mut queue = vec![t.name.clone()];
    while let Some(name) = queue.pop() {
        for found in types.iter().filter(|x| x.extends.as_deref() == Some(name.as_str()) || x.implements.contains(&name)) {
            if !out.iter().any(|o| o.key == found.key) {
                out.push(found);
                queue.push(found.name.clone());
            }
        }
    }
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
        .flat_map(|t| t.methods.iter().filter(|(n, _, _)| methods.iter().any(|(m, _)| m == n)).map(|(_, loc, _)| loc.clone()))
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
