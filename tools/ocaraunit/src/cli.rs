// ─────────────────────────────────────────────────────────────────────────────
// Ligne de commande et détection de la racine du projet
// ─────────────────────────────────────────────────────────────────────────────

use std::path::{Path, PathBuf};

use crate::config::find_config_root;

/// Délai par défaut d'un fichier de test (compilation, puis exécution).
pub const DEFAULT_TIMEOUT_S: u64 = 120;

#[derive(Debug, PartialEq)]
pub struct Options {
    pub coverage:        bool,
    /// `--coverage=<dossier|fichier>` : périmètre de la couverture
    pub coverage_target: Option<String>,
    /// Dossier ou fichier de tests (défaut : `tests/`)
    pub target:          Option<String>,
    pub src:             Option<String>,
    pub clear:           bool,
    pub timeout_s:       u64,
    pub help:            bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            coverage: false, coverage_target: None, target: None, src: None,
            clear: false, timeout_s: DEFAULT_TIMEOUT_S, help: false,
        }
    }
}

fn parse_timeout(value: &str) -> Result<u64, String> {
    match value.parse::<u64>() {
        Ok(s) if s > 0 => Ok(s),
        _ => Err(format!("--timeout attend un nombre de secondes > 0 (reçu « {} »)", value)),
    }
}

/// Valeur d'une option : `--opt=valeur` ou `--opt valeur`.
fn value(inline: Option<&str>, rest: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    match inline {
        Some(v) if !v.is_empty() => Ok(v.to_string()),
        Some(_) => Err(format!("{} nécessite un argument", name)),
        None => rest.next().ok_or_else(|| format!("{} nécessite un argument", name)),
    }
}

/// Arguments positionnels : (valeur, suit directement `--coverage`).
type Positionals = Vec<(String, bool)>;

/// Répartit les arguments positionnels : un seul → dossier/fichier de
/// tests (même juste après `--coverage`, qui l'avalait autrefois) ; deux →
/// celui qui suit `--coverage` est le périmètre de couverture, l'autre les
/// tests (`--coverage src/Foo.oc tests/FooTest.oc`, utilisé par la CI).
fn assign_positionals(opts: &mut Options, mut pos: Positionals) -> Result<(), String> {
    match pos.len() {
        0 => {}
        1 => opts.target = Some(pos.remove(0).0),
        2 if pos.iter().filter(|(_, cov)| *cov).count() == 1 && opts.coverage_target.is_none() => {
            let idx = pos.iter().position(|(_, cov)| *cov).unwrap_or(0);
            opts.coverage_target = Some(pos.remove(idx).0);
            opts.target = Some(pos.remove(0).0);
        }
        _ => {
            let extra: Vec<String> = pos.into_iter().map(|(v, _)| v).collect();
            return Err(format!("un seul dossier ou fichier de tests attendu (reçu : {})", extra.join(", ")));
        }
    }
    Ok(())
}

/// Analyse les arguments (sans le nom du programme). Option inconnue ou
/// argument positionnel en trop : erreur, plutôt qu'un oubli silencieux.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut positionals: Positionals = Vec::new();
    let mut after_coverage = false;
    let mut rest = args.into_iter();
    while let Some(arg) = rest.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v)),
            _ => (arg.clone(), None),
        };
        let follows_coverage = std::mem::take(&mut after_coverage);
        match name.as_str() {
            "--coverage" => {
                opts.coverage = true;
                match inline {
                    Some(_) => opts.coverage_target = Some(value(inline, &mut rest, "--coverage")?),
                    None => after_coverage = true,
                }
            }
            "--src" => opts.src = Some(value(inline, &mut rest, "--src")?),
            "--timeout" => opts.timeout_s = parse_timeout(&value(inline, &mut rest, "--timeout")?)?,
            "--clear" if inline.is_none() => opts.clear = true,
            "--help" | "-h" if inline.is_none() => opts.help = true,
            other if other.starts_with('-') => return Err(format!("option inconnue : {}", arg)),
            _ => positionals.push((arg, follows_coverage)),
        }
    }
    assign_positionals(&mut opts, positionals)?;
    Ok(opts)
}

/// Racine du projet d'un chemin de tests (dossier, ou fichier → son
/// dossier) : parent du dossier `tests` le plus proche, sinon dossier du
/// `.ocaraunit` le plus proche, sinon le dossier lui-même.
pub fn detect_project_root(path: &Path) -> PathBuf {
    let base = if path.is_file() { path.parent().unwrap_or(path) } else { path };
    if let Some(tests) = base.ancestors().find(|a| a.file_name().is_some_and(|n| n == "tests")) {
        if let Some(parent) = tests.parent() {
            return parent.to_path_buf();
        }
    }
    find_config_root(base).unwrap_or_else(|| base.to_path_buf())
}
