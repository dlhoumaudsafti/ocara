//! Index de l'espace de travail pour les références et les CodeLens : chaque
//! projet (`main.oc`) est analysé depuis son point d'entrée, chaque fichier
//! qu'aucun projet n'importe l'est seul. Les références de la sema sont
//! groupées par déclaration ; la hiérarchie des types vient du programme
//! fusionné de chaque entrée (deux classes homonymes de projets différents
//! ne sont pas confondues).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use lsp_types::{Location, Range};

use super::decls;
use super::features::{span_path, to_url};
use super::position::name_range;
use super::workspace::Workspace;
use crate::parsing::ast::{ClassMember, Program};
use crate::parsing::token::Span;
use crate::sema::index::Target;

/// Déclaration : fichier, ligne et colonne de son span.
pub type DeclKey = (PathBuf, usize, usize);

pub fn decl_key(entry: &Path, span: &Span) -> DeclKey {
    (span_path(span, entry), span.line, span.col)
}

#[derive(Clone)]
pub struct TypeDecl {
    pub name: String,
    pub key: DeclKey,
    pub location: Location,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub modules: Vec<String>,
    /// Méthodes déclarées (nom, emplacement du nom, clé de la déclaration).
    pub methods: Vec<(String, Location, DeclKey)>,
}

struct Entry {
    files: HashSet<PathBuf>,
    references: Vec<(DeclKey, Location)>,
    types: Vec<TypeDecl>,
}

#[derive(Default)]
pub struct ProjectIndex {
    entries: HashMap<PathBuf, Entry>,
    built: bool,
    dirty: HashSet<PathBuf>,
}

impl ProjectIndex {
    /// Le document `path` a changé : ses entrées seront réanalysées.
    pub fn invalidate(&mut self, path: &Path) {
        let owners: Vec<PathBuf> = self.entries.iter()
            .filter(|(_, e)| e.files.contains(path))
            .map(|(k, _)| k.clone())
            .collect();
        if owners.is_empty() {
            self.dirty.insert(path.to_path_buf());
        }
        self.dirty.extend(owners);
    }

    pub fn ensure(&mut self, ws: &Workspace) {
        if !self.built {
            self.build(ws);
            self.built = true;
            self.dirty.clear();
        }
        // Une entrée qui ne s'analyse plus (syntaxe en cours de frappe) garde
        // ses données précédentes ; un fichier supprimé perd la sienne.
        for entry in std::mem::take(&mut self.dirty) {
            if ws.text(&entry).is_some() {
                self.add_entry(ws, &entry);
            } else {
                self.entries.remove(&entry);
            }
        }
    }

    fn build(&mut self, ws: &Workspace) {
        let mut files = Vec::new();
        for root in ws.roots() {
            collect_sources(root, &mut files);
        }
        files.sort();
        let (mains, others): (Vec<_>, Vec<_>) = files.into_iter().partition(|f| f.file_name().is_some_and(|n| n == "main.oc"));
        for main in &mains {
            self.add_entry(ws, main);
        }
        for file in &others {
            if !self.entries.values().any(|e| e.files.contains(file)) {
                self.add_entry(ws, file);
            }
        }
    }

    fn add_entry(&mut self, ws: &Workspace, entry: &Path) {
        let analysis = ws.run(entry);
        let Some(checked) = analysis.checked else { return };
        let program = &checked.program;
        let mut texts: HashMap<PathBuf, String> = HashMap::new();
        let mut text_of = |file: &Path| -> String {
            texts.entry(file.to_path_buf()).or_insert_with(|| ws.text(file).unwrap_or_default()).clone()
        };

        let mut references = Vec::new();
        let mut files = HashSet::from([entry.to_path_buf()]);
        for r in &checked.references {
            let key = match &r.target {
                Target::Completion(_) => continue,
                Target::Local { decl } => decl_key(entry, decl),
                target => match decls::find(program, target) {
                    Some(d) => decl_key(entry, &d.span),
                    None => continue,
                },
            };
            let file = span_path(&r.span, entry);
            let range = name_range(&text_of(&file), &r.span, &r.name);
            files.insert(file.clone());
            references.push((key, Location::new(to_url(&file), range)));
        }
        let types = type_decls(program, entry, &mut text_of);
        files.extend(types.iter().map(|t| t.key.0.clone()));

        // Noms de type écrits dans chaque fichier (annotations, `extends`…).
        for file in &files {
            let text = text_of(file);
            let Some(own) = parse(&text) else { continue };
            for (name, span) in &own.type_refs {
                let Some(d) = decls::find(program, &Target::Class(name.clone())) else { continue };
                references.push((decl_key(entry, &d.span), Location::new(to_url(file), name_range(&text, span, name))));
            }
        }
        self.entries.insert(entry.to_path_buf(), Entry { files, references, types });
    }

    /// Tous les fichiers couverts par l'index.
    pub fn files(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = self.entries.values().flat_map(|e| e.files.iter().cloned()).collect();
        out.sort();
        out.dedup();
        out
    }

    pub fn references(&self, key: &DeclKey) -> Vec<Location> {
        let mut out: Vec<Location> = Vec::new();
        for entry in self.entries.values() {
            for (k, loc) in &entry.references {
                if k == key && !out.contains(loc) {
                    out.push(loc.clone());
                }
            }
        }
        out
    }

    /// Types vus par les entrées qui contiennent `file`.
    pub fn types_for(&self, file: &Path) -> Vec<Vec<TypeDecl>> {
        self.entries.values().filter(|e| e.files.contains(file)).map(|e| e.types.clone()).collect()
    }
}

fn type_decls(program: &Program, entry: &Path, text_of: &mut dyn FnMut(&Path) -> String) -> Vec<TypeDecl> {
    let mut out = Vec::new();
    let mut location = |span: &Span, name: &str| {
        let file = span_path(span, entry);
        Location::new(to_url(&file), name_range(&text_of(&file), span, name))
    };
    let method = |name: &str, span: &Span, location: &mut dyn FnMut(&Span, &str) -> Location| {
        (name.to_string(), location(span, name), decl_key(entry, span))
    };
    let methods_of = |members: &[ClassMember], location: &mut dyn FnMut(&Span, &str) -> Location| -> Vec<(String, Location, DeclKey)> {
        members.iter().filter_map(|m| match m {
            ClassMember::Method { decl, .. } => Some(method(&decl.name, &decl.span, location)),
            _ => None,
        }).collect()
    };
    for c in &program.classes {
        let methods = methods_of(&c.members, &mut location);
        out.push(TypeDecl {
            name: c.name.clone(), key: decl_key(entry, &c.span), location: location(&c.span, &c.name),
            extends: c.extends.clone(), implements: c.implements.clone(), modules: c.modules.clone(), methods,
        });
    }
    for g in &program.generics {
        let methods = methods_of(&g.members, &mut location);
        out.push(TypeDecl {
            name: g.name.clone(), key: decl_key(entry, &g.span), location: location(&g.span, &g.name),
            extends: g.extends.clone(), implements: g.implements.clone(), modules: g.modules.clone(), methods,
        });
    }
    for i in &program.interfaces {
        let methods = i.methods.iter().map(|m| (m.name.clone(), location(&m.span, &m.name), decl_key(entry, &m.span))).collect();
        out.push(TypeDecl {
            name: i.name.clone(), key: decl_key(entry, &i.span), location: location(&i.span, &i.name),
            extends: None, implements: Vec::new(), modules: Vec::new(), methods,
        });
    }
    out
}

pub fn parse(text: &str) -> Option<Program> {
    let tokens = crate::parsing::lexer::Lexer::new(text).tokenize().ok()?;
    crate::parsing::parser::Parser::new(tokens).parse_program().ok()
}

fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !matches!(name.as_ref(), "target" | "node_modules" | "out" | ".git") {
                collect_sources(&path, out);
            }
        } else if name.ends_with(".oc") {
            out.push(path.canonicalize().unwrap_or(path));
        }
    }
}

/// Plage vide au début d'un emplacement (position d'un CodeLens).
pub fn start_of(location: &Location) -> Range {
    Range::new(location.range.start, location.range.start)
}
