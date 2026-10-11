//! Types homonymes de fichiers différents (`Format` du contexte `home` et
//! `Format` du contexte `car`) : chacun reçoit un nom interne unique tiré de
//! son namespace, et chaque fichier voit celui qu'il déclare ou importe.
//! Les noms sont réécrits par fichier avant la fusion des imports (voir
//! docs/roadmap.d/langage-classes-homonymes-fusionnees.md) ; seuls les noms
//! réellement en collision sont touchés.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::imports::{parse_module, resolve_import_path, virtual_from};
use crate::core::alias_resolve::resolve_aliases;
use crate::parsing::ast::Program;

/// Séparateur du nom interne : `Format__ns__context_car_app_usecase_helpers`.
pub const NS_MARK: &str = "__ns__";

#[derive(Default)]
pub struct Homonyms {
    /// Par fichier : nom écrit → nom interne, pour ce qu'il déclare ou importe.
    views: HashMap<PathBuf, HashMap<String, String>>,
    /// Par fichier : ses propres déclarations renommées.
    declared: HashMap<PathBuf, HashMap<String, String>>,
}

struct FileInfo {
    namespace: Option<String>,
    declared: Vec<String>,
    /// (symbole importé — `None` pour `*` —, fichier visé)
    imports: Vec<(Option<String>, PathBuf)>,
}

fn key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn type_names(program: &Program) -> Vec<String> {
    program.classes.iter().map(|c| c.name.clone())
        .chain(program.generics.iter().map(|g| g.name.clone()))
        .chain(program.interfaces.iter().map(|i| i.name.clone()))
        .chain(program.modules.iter().map(|m| m.name.clone()))
        .chain(program.enums.iter().map(|e| e.name.clone()))
        .collect()
}

impl Homonyms {
    /// Parcourt le graphe des imports depuis le fichier d'entrée (mêmes règles
    /// de résolution que `imports::load_imports`). Un fichier illisible est
    /// ignoré ici : `load_imports` rapportera l'erreur.
    pub fn scan(entry: &Program, input: &Path, source_dir: &Path) -> Self {
        let mut files: Vec<(PathBuf, FileInfo)> = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut queue: Vec<(PathBuf, Program)> = vec![(key(input), entry.clone())];
        seen.insert(key(input));
        let entry_key = key(input);
        while let Some((path, program)) = queue.pop() {
            // Comme `load_imports` : les `import X from "…"` du fichier
            // d'entrée partent du dossier source, ceux des autres de leur dossier.
            let dir = if path == entry_key { source_dir.to_path_buf() } else { path.parent().unwrap_or(source_dir).to_path_buf() };
            let mut imports = Vec::new();
            for imp in program.imports.iter().filter(|i| i.path.first().is_some_and(|p| p != "ocara")) {
                let (imp, parent) = match &imp.file_path {
                    Some(_) => (imp.clone(), dir.clone()),
                    None => (virtual_from(imp), source_dir.to_path_buf()),
                };
                let target = key(&resolve_import_path(imp.file_path.as_ref().unwrap(), &parent, &program.namespace, source_dir));
                let symbol = imp.path.first().filter(|s| *s != "*").cloned();
                imports.push((symbol, target.clone()));
                if seen.insert(target.clone()) {
                    if let Ok(parsed) = parse_module(&target) {
                        queue.push((target, parsed));
                    }
                }
            }
            files.push((path, FileInfo { namespace: program.namespace.clone(), declared: type_names(&program), imports }));
        }

        let mut declaring: HashMap<&str, Vec<&Path>> = HashMap::new();
        for (path, info) in &files {
            for name in &info.declared {
                let list = declaring.entry(name).or_default();
                if !list.contains(&path.as_path()) {
                    list.push(path);
                }
            }
        }
        let mut internal: HashMap<(PathBuf, String), String> = HashMap::new();
        let mut used: HashSet<String> = HashSet::new();
        for (name, paths) in declaring.iter().filter(|(_, p)| p.len() > 1) {
            for path in paths {
                let info = &files.iter().find(|(p, _)| p == path).unwrap().1;
                let base = format!("{}{}{}", name, NS_MARK, qualifier(info.namespace.as_deref(), path, source_dir));
                let mut unique = base.clone();
                let mut n = 2;
                while !used.insert(unique.clone()) {
                    unique = format!("{}_{}", base, n);
                    n += 1;
                }
                internal.insert((path.to_path_buf(), name.to_string()), unique);
            }
        }

        let mut out = Homonyms::default();
        if internal.is_empty() {
            return out;
        }
        for (path, info) in &files {
            let mut view = HashMap::new();
            for (symbol, target) in &info.imports {
                let names: Vec<String> = match symbol {
                    Some(s) => vec![s.clone()],
                    None => files.iter().find(|(p, _)| p == target).map(|(_, i)| i.declared.clone()).unwrap_or_default(),
                };
                for name in names {
                    if let Some(int) = internal.get(&(target.clone(), name.clone())) {
                        view.insert(name, int.clone());
                    }
                }
            }
            let mut own = HashMap::new();
            for name in &info.declared {
                if let Some(int) = internal.get(&(path.clone(), name.clone())) {
                    own.insert(name.clone(), int.clone());
                    view.insert(name.clone(), int.clone());
                }
            }
            if !view.is_empty() {
                out.views.insert(path.clone(), view);
            }
            if !own.is_empty() {
                out.declared.insert(path.clone(), own);
            }
        }
        out
    }

    /// Renomme les déclarations homonymes du fichier `path` et réécrit ses
    /// références selon ce qu'il déclare et importe.
    pub fn apply(&self, program: &mut Program, path: &Path) {
        let path = key(path);
        if let Some(view) = self.views.get(&path) {
            resolve_aliases(program, view);
        }
        let Some(own) = self.declared.get(&path) else { return };
        let rename = |name: &mut String| if let Some(int) = own.get(name.as_str()) { *name = int.clone(); };
        program.classes.iter_mut().for_each(|c| rename(&mut c.name));
        program.generics.iter_mut().for_each(|g| rename(&mut g.name));
        program.interfaces.iter_mut().for_each(|i| rename(&mut i.name));
        program.modules.iter_mut().for_each(|m| rename(&mut m.name));
        program.enums.iter_mut().for_each(|e| rename(&mut e.name));
    }

    /// Nom interne d'un symbole déclaré par `path` (import sélectif).
    pub fn internal(&self, path: &Path, name: &str) -> Option<String> {
        self.declared.get(&key(path))?.get(name).cloned()
    }

    /// Nom écrit → nom interne, vu depuis `path` (serveur de langage).
    pub fn view(&self, path: &Path) -> Option<&HashMap<String, String>> {
        self.views.get(&key(path))
    }
}

/// Namespace du fichier (`context.car.app` → `context_car_app`), ou son chemin
/// relatif sans extension s'il n'en déclare pas.
fn qualifier(namespace: Option<&str>, path: &Path, source_dir: &Path) -> String {
    let raw = match namespace.filter(|n| *n != ".") {
        Some(ns) => ns.to_string(),
        None => path.strip_prefix(key(source_dir)).unwrap_or(path).with_extension("").to_string_lossy().into_owned(),
    };
    raw.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect()
}

/// Message lisible : noms internes réduits au nom écrit.
pub fn display(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(NS_MARK) {
        out.push_str(&rest[..i]);
        rest = &rest[i + NS_MARK.len()..];
        let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}
