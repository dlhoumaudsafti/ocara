//! Documents ouverts et analyse de chacun avec le texte de l'éditeur.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::analysis::{analyze, Analysis, AnalyzeOptions};
use crate::core::source;

#[derive(Default)]
pub struct Workspace {
    /// Dossiers racine du client (`workspaceFolders`).
    roots: Vec<PathBuf>,
    /// Texte courant de chaque document ouvert.
    texts: HashMap<PathBuf, String>,
    /// Dernière analyse de chaque document ouvert.
    analyses: HashMap<PathBuf, Analysis>,
}

impl Workspace {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots, ..Self::default() }
    }

    pub fn open(&mut self, path: PathBuf, text: String) {
        source::set_override(&path, text.clone());
        self.texts.insert(path, text);
    }

    pub fn close(&mut self, path: &Path) {
        source::clear_override(path);
        self.texts.remove(path);
        self.analyses.remove(path);
    }

    pub fn open_paths(&self) -> Vec<PathBuf> {
        self.texts.keys().cloned().collect()
    }

    /// Texte d'un fichier : version de l'éditeur s'il est ouvert, disque sinon.
    pub fn text(&self, path: &Path) -> Option<String> {
        self.texts.get(path).cloned().or_else(|| source::read(path).ok())
    }

    pub fn analysis(&self, path: &Path) -> Option<&Analysis> {
        self.analyses.get(path)
    }

    /// Analyse `path` comme fichier d'entrée, imports résolus depuis la racine
    /// du projet ; les gabarits `renderFile` sont lus depuis cette racine.
    pub fn analyze(&mut self, path: &Path) -> &Analysis {
        let root = self.project_root(path);
        let _ = std::env::set_current_dir(&root);
        let analysis = analyze(&AnalyzeOptions { input: path, src_dir: Some(&root), dump: false, index: true });
        self.analyses.insert(path.to_path_buf(), analysis);
        &self.analyses[path]
    }

    /// Premier dossier contenant un `main.oc` en remontant depuis le fichier,
    /// sans sortir du dossier racine du client ; dossier du fichier sinon.
    pub fn project_root(&self, path: &Path) -> PathBuf {
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let limit = self.roots.iter().find(|r| path.starts_with(r));
        for ancestor in dir.ancestors() {
            if ancestor.join("main.oc").is_file() {
                return ancestor.to_path_buf();
            }
            if limit.is_some_and(|r| ancestor == r.as_path()) {
                break;
            }
        }
        dir
    }
}
