//! Architecture hexagonale (voir docs/roadmap.d/langage-mode-hexa.md) :
//! configuration (bloc `architecture` du fichier analysé, sinon du `main.oc`
//! du projet, sinon défaut `src: src`), classement des fichiers du
//! répertoire `src` en `<src>/<contexte>/<couche>/…`, et vérification des
//! imports et `wiring` de chaque fichier atteint depuis l'entrée. Règles :
//! architecture_rules.rs.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::architecture_rules::{check_file, Place};
use super::imports::parse_module;
use crate::core::diagnostics::Diagnostic;
use crate::parsing::ast::{ArchitectureDecl, ArchitectureMode, Program};
use crate::parsing::token::Span;

pub struct Config {
    pub root: PathBuf,
    pub src: PathBuf,
    /// Répertoire hexagonal tel qu'écrit (`context`).
    pub src_name: String,
    /// alias de répertoire → couche canonique.
    pub layers: HashMap<String, &'static str>,
    pub shared: Vec<PathBuf>,
}

impl Config {
    /// Place d'un fichier : hors `src`, non classable, ou contexte + couche.
    /// Une racine `shared` hors de `src` est son propre contexte.
    pub fn place(&self, file: &Path) -> Place {
        let (base, context) = match file.strip_prefix(&self.src) {
            Ok(rel) => (rel.to_path_buf(), None),
            Err(_) => match self.shared.iter().find(|s| file.starts_with(s)) {
                Some(s) => (file.strip_prefix(s).unwrap_or(file).to_path_buf(), Some(self.dotted(s))),
                None => return Place::Outside,
            },
        };
        let dirs: Vec<String> = base.parent().map(|p| p.iter().map(|c| c.to_string_lossy().into_owned()).collect()).unwrap_or_default();
        let (context, layer_dir) = match context {
            Some(c) => (c, dirs.first()),
            None if dirs.len() >= 2 => (dirs[0].clone(), dirs.get(1)),
            None => return Place::Unclassified,
        };
        match layer_dir.and_then(|d| self.layers.get(d)) {
            Some(layer) => Place::Layer { context, layer },
            None => Place::Unclassified,
        }
    }

    pub fn is_shared(&self, file: &Path) -> bool {
        self.shared.iter().any(|s| file.starts_with(s))
    }

    /// Chemin pointé depuis la racine du projet (`context.car.infra.db.Foo`).
    pub fn dotted(&self, path: &Path) -> String {
        let rel = path.strip_prefix(&self.root).unwrap_or(path).with_extension("");
        rel.iter().map(|c| c.to_string_lossy()).collect::<Vec<_>>().join(".")
    }

    pub fn alias(&self, layer: &str) -> String {
        self.layers.iter().find(|(_, l)| **l == layer).map(|(a, _)| a.clone()).unwrap_or_default()
    }
}

pub fn check(entry: &Program, input: &Path, root: &Path) -> Vec<Diagnostic> {
    // `ocara main.oc` : dossier du fichier = chemin vide.
    let root = if root.as_os_str().is_empty() { Path::new(".") } else { root };
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let project_main = root.join("main.oc");
    let (decl, decl_file) = match &entry.architecture {
        Some(d) => (Some(d.clone()), input.to_path_buf()),
        None if project_main.is_file() && !same(&project_main, input) => {
            (parse_module(&project_main).ok().and_then(|p| p.architecture), project_main.clone())
        }
        None => (None, input.to_path_buf()),
    };
    if decl.as_ref().is_some_and(|d| d.mode == ArchitectureMode::Permissive) {
        return Vec::new();
    }
    let config = match configure(decl.as_ref(), &decl_file, &root) {
        Ok(config) => config,
        Err(diags) => return diags,
    };
    let mut out = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut queue: Vec<(PathBuf, Option<Program>)> = vec![(input.canonicalize().unwrap_or_else(|_| input.to_path_buf()), Some(entry.clone()))];
    while let Some((file, program)) = queue.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        let Some(program) = program.or_else(|| parse_module(&file).ok()) else { continue };
        let entry_file = seen.len() == 1;
        for next in check_file(&config, &file, &program, entry_file, &mut out) {
            if !seen.contains(&next) {
                queue.push((next, None));
            }
        }
    }
    out
}

/// Configuration effective ; erreurs de configuration sinon.
fn configure(decl: Option<&ArchitectureDecl>, file: &Path, root: &Path) -> Result<Config, Vec<Diagnostic>> {
    let at = |span: Option<&Span>, msg: String| match span {
        Some(s) => Diagnostic::error(file, s.line, s.col, msg),
        None => Diagnostic::error(file, 1, 1, msg),
    };
    let (src_name, src_span) = match decl.and_then(|d| d.src.as_ref()) {
        Some((name, span)) => (name.clone(), Some(span)),
        None => ("src".to_string(), decl.map(|d| &d.span)),
    };
    let mut errors = Vec::new();
    let src = root.join(src_name.replace('.', "/"));
    if !src.is_dir() {
        let declared = if decl.is_some() { "" } else { " (architecture hexagonale par défaut)" };
        errors.push(at(src_span, format!("répertoire hexagonal '{}/' introuvable{} : structurez le projet en '{}/<contexte>/<couche>' ou déclarez 'architecture permissive'", src_name.replace('.', "/"), declared, src_name.replace('.', "/"))));
    }
    let mut layers: HashMap<String, &'static str> = HashMap::new();
    for canonical in ["infrastructure", "application", "domain"] {
        let declared = decl.and_then(|d| d.layers.iter().find(|(l, _, _)| l == canonical));
        let alias = declared.map(|(_, a, _)| a.clone()).unwrap_or_else(|| canonical.to_string());
        if layers.insert(alias.clone(), canonical).is_some() {
            errors.push(at(declared.map(|(_, _, s)| s), format!("architecture : alias de couche '{}' utilisé deux fois", alias)));
        }
    }
    let mut shared = Vec::new();
    for (path, span) in decl.map(|d| d.shared.as_slice()).unwrap_or_default() {
        let dir = root.join(path.replace('.', "/"));
        if !dir.is_dir() {
            errors.push(at(Some(span), format!("architecture : racine partagée '{}' introuvable", path)));
        } else if !layers.keys().any(|alias| dir.join(alias).is_dir()) {
            errors.push(at(Some(span), format!("architecture : racine partagée '{}' sans segment de couche ({}) — seul un répertoire structuré en couches peut être partagé", path, layers.keys().cloned().collect::<Vec<_>>().join(", "))));
        } else {
            shared.push(dir.canonicalize().unwrap_or(dir));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let src = src.canonicalize().unwrap_or(src);
    Ok(Config { root: root.to_path_buf(), src, src_name, layers, shared })
}

fn same(a: &Path, b: &Path) -> bool {
    a.canonicalize().ok().zip(b.canonicalize().ok()).is_some_and(|(a, b)| a == b)
}
