// ─────────────────────────────────────────────────────────────────────────────
// Configuration (.ocaraunit) et recherche des fichiers .oc
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Default)]
pub struct Config {
    /// Répertoires ou fichiers exclus (tests et couverture)
    pub exclude: Vec<String>,
}

/// Premier dossier contenant un `.ocaraunit`, en remontant depuis `start`.
pub fn find_config_root(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|d| d.join(".ocaraunit").exists()).map(Path::to_path_buf)
}

pub fn load_config(root: &Path) -> Config {
    match fs::read_to_string(root.join(".ocaraunit")) {
        Ok(content) => parse_config(&content),
        Err(_) => Config::default(),
    }
}

pub fn parse_config(content: &str) -> Config {
    let mut cfg = Config::default();
    let mut in_exclude = false;
    for raw in content.lines() {
        let line = raw.trim();
        if line == "[exclude]"      { in_exclude = true;  continue; }
        if line.starts_with('[')    { in_exclude = false; continue; }
        if line.is_empty() || line.starts_with('#') { continue; }
        if in_exclude {
            cfg.exclude.push(line.to_string());
        }
    }
    cfg
}

/// Composants « normaux » d'un chemin (`./`, `/` final ignorés).
fn parts(path: &Path) -> Vec<&std::ffi::OsStr> {
    path.components()
        .filter_map(|c| match c { Component::Normal(s) => Some(s), _ => None })
        .collect()
}

/// Exclusion par composants de chemin : `src/` exclut `src/…` et `a/src/…`,
/// jamais `mysrc/` (l'ancienne comparaison par sous-chaîne le faisait).
pub fn is_excluded(path: &Path, root: &Path, excludes: &[String]) -> bool {
    let rel = parts(path.strip_prefix(root).unwrap_or(path));
    excludes.iter().any(|ex| {
        let ex = parts(Path::new(ex.as_str()));
        !ex.is_empty() && rel.windows(ex.len()).any(|w| w == ex.as_slice())
    })
}

pub fn collect_oc_files(dir: &Path, excludes: &[String], root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else { return files };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if is_excluded(&path, root, excludes) { continue; }
        if path.is_dir() {
            files.extend(collect_oc_files(&path, excludes, root));
        } else if path.extension().and_then(|e| e.to_str()) == Some("oc") {
            files.push(path);
        }
    }
    files
}

pub fn is_test_file(path: &Path) -> bool {
    path.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.ends_with("Test"))
}

pub fn collect_test_files(dir: &Path, excludes: &[String], root: &Path) -> Vec<PathBuf> {
    collect_oc_files(dir, excludes, root).into_iter().filter(|p| is_test_file(p)).collect()
}
