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

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
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
    /// Une erreur avant la sema (syntaxe en cours de frappe) garde le dernier
    /// programme vérifié : survol et définition restent disponibles.
    pub fn analyze(&mut self, path: &Path) -> &Analysis {
        let mut analysis = self.run(path);
        if analysis.checked.is_none() {
            analysis.checked = self.analyses.remove(path).and_then(|previous| previous.checked);
        }
        self.analyses.insert(path.to_path_buf(), analysis);
        &self.analyses[path]
    }

    /// Analyse ponctuelle de `path` avec le texte `text` (complétion), sans
    /// toucher à la dernière analyse du document.
    pub fn analyze_text(&self, path: &Path, text: String) -> Analysis {
        source::set_override(path, text);
        let analysis = self.run(path);
        match self.texts.get(path) {
            Some(current) => source::set_override(path, current.clone()),
            None => source::clear_override(path),
        }
        analysis
    }

    /// Analyse de `path` avec le texte courant des documents ouverts.
    pub fn run(&self, path: &Path) -> Analysis {
        let root = self.project_root(path);
        let _ = std::env::set_current_dir(&root);
        let input = self.entry_for(path, &root);
        analyze(&AnalyzeOptions { input: &input, src_dir: Some(&root), dump: false, index: true, tolerant: true })
    }

    /// Fichier d'entrée de l'analyse de `path` : le `main.oc` du projet pour
    /// un fichier runtime qu'il importe (`runtime core.main is main` — seul,
    /// son contenu n'est pas un programme), `path` lui-même sinon.
    pub fn entry_for(&self, path: &Path, root: &Path) -> PathBuf {
        let main = root.join("main.oc");
        let imports_it = self.text(&main)
            .and_then(|text| super::project::parse(&text))
            .is_some_and(|program| program.runtime_imports.iter().any(|rt| {
                crate::core::runtime_expand::resolve_runtime_file(root, &rt.path)
                    .is_some_and(|f| f.canonicalize().unwrap_or(f) == path)
            }));
        if imports_it { main.canonicalize().unwrap_or(main) } else { path.to_path_buf() }
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
