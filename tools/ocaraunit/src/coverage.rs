// ─────────────────────────────────────────────────────────────────────────────
// Analyse de couverture (par nom : `foo` couverte par `fooTest`/`testFoo`)
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::discovery::declarations;

#[derive(Debug)]
pub struct FileCoverage {
    pub path:          PathBuf,
    pub total_funcs:   usize,
    pub covered_funcs: usize,
    /// Fonctions non couvertes : (nom, numéro de ligne 1-basé)
    pub uncovered:     Vec<(String, usize)>,
}

impl FileCoverage {
    pub fn percent(&self) -> f64 {
        if self.total_funcs == 0 { return 100.0; }
        (self.covered_funcs as f64 / self.total_funcs as f64) * 100.0
    }
}

/// Clé de comparaison d'un nom : minuscules, sans `_` (`add_bonus` est
/// couverte par `addBonusTest`).
pub fn normalize(name: &str) -> String {
    name.chars().filter(|&c| c != '_').flat_map(char::to_lowercase).collect()
}

/// Fonctions couvertes avec leur numéro de ligne : les méthodes (hors `init`)
/// si le fichier déclare une classe, sinon les fonctions libres (hors
/// `main`), hors déclarations `async` — règle historique, dont dépend l'exigence de 100 % de la CI
/// (`ci/start.sh`) : dans un fichier à classes, les fonctions libres sont
/// des aides de démonstration, pas l'API testée.
pub fn extract_all_function_names(source: &str) -> Vec<(String, usize)> {
    let has_class = source.lines().any(|l| {
        let t = l.trim();
        t.starts_with("class ") || t == "class"
    });
    let lines: Vec<&str> = source.lines().collect();
    // Déclarations `async` exclues, comme avant (elles ne correspondaient à
    // aucun préfixe reconnu).
    let is_async = |line: usize| lines.get(line - 1).is_some_and(|l| l.split_whitespace().any(|w| w == "async"));
    declarations(source).into_iter()
        .filter(|d| if has_class { d.is_method && d.name != "init" } else { !d.is_method && d.name != "main" })
        .filter(|d| !is_async(d.line))
        .map(|d| (d.name, d.line))
        .collect()
}

/// Couverture d'un fichier source non-test : « couverte » = un test
/// s'appelle `<nom>Test` ou `test<Nom>` (casse et `_` ignorés ;
/// `test_names` est déjà normalisé).
pub fn compute_coverage(src_file: &Path, all_funcs: &[(String, usize)], test_names: &HashSet<String>) -> FileCoverage {
    let mut uncovered = Vec::new();
    for (name, lineno) in all_funcs {
        let lower = normalize(name);
        if !test_names.contains(&format!("{}test", lower)) && !test_names.contains(&format!("test{}", lower)) {
            uncovered.push((name.clone(), *lineno));
        }
    }
    FileCoverage {
        path:          src_file.to_path_buf(),
        total_funcs:   all_funcs.len(),
        covered_funcs: all_funcs.len() - uncovered.len(),
        uncovered,
    }
}
