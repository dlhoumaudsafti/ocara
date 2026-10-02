// ─────────────────────────────────────────────────────────────────────────────
// ocaracs — analyseur de style pour Ocara
//
// Usage :
//   ocaracs [--fix] <fichier.oc>
//   ocaracs [--fix] <dossier>
//
// Configuration : fichier .ocaracs à la racine du projet (TOML simplifié).
//
// Format de sortie (compatible VS Code / GCC / clang, cliquable) :
//   fichier.oc:LIGNE:COL: warning: message
// ─────────────────────────────────────────────────────────────────────────────

mod check;
mod config;
mod decls;
mod fix;
mod naming;
mod rename;
mod scope;
mod text;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use config::{find_project_root, load_config, Config};

// ─────────────────────────────────────────────────────────────────────────────
// Fichiers analysés : un fichier et ses imports utilisateur, ou un dossier
// ─────────────────────────────────────────────────────────────────────────────

const OCARA_BUILTINS: &[&str] = &[
    "IO", "Math", "String", "Array", "Map", "JSON", "Tauri", "SDL",
    "Convert", "System", "Regex", "HTTPRequest", "HTTPServer", "HTTPServerRequest", "HTTPServerSession", "SQLite", "MySQL", "MariaDB", "DotEnv", "YAML", "Thread", "Mutex",
    "DateTime", "Date", "Time", "UnitTest", "HTMLComponent", "HTML",
    "File", "Directory", "Exception", "FileException", "DirectoryException", "IOException", "SystemException",
    "ArrayException", "MapException", "MathException", "ConvertException", "RegexException",
    "DateTimeException", "DateException", "TimeException",
    "ThreadException", "MutexException",
    "UnitTestException", "HTTPServerException", "SQLiteException", "MySQLException", "MariaDBException", "DotEnvException", "YAMLException",
    "TauriException", "SDLException",
];

fn extract_user_imports(content: &str, file_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if !t.starts_with("import ") { continue; }
        // Supprimer la partie " as Alias" éventuelle
        let rest_raw = t[7..].trim();
        let rest = rest_raw.split(" as ").next().unwrap_or(rest_raw).trim();
        if rest.starts_with("ocara.") || rest == "ocara.*" { continue; }
        let first = rest.split('.').next().unwrap_or("");
        if OCARA_BUILTINS.contains(&first) { continue; }
        let mut path = file_dir.to_path_buf();
        for seg in rest.split('.') { path.push(seg); }
        path.set_extension("oc");
        out.push(path);
    }
    out
}

/// Ajoute `path` (canonique) et, récursivement, ses imports utilisateur.
fn collect_with_imports(path: &Path, out: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    // Un import inexistant est ignoré silencieusement (import de démonstration).
    let Ok(canonical) = path.canonicalize() else { return };
    if !seen.insert(canonical.clone()) { return; }
    let Ok(content) = fs::read_to_string(&canonical) else {
        eprintln!("ocaracs: impossible de lire '{}'", path.display());
        return;
    };
    out.push(canonical.clone());
    let file_dir = canonical.parent().unwrap_or(Path::new("."));
    for imp in extract_user_imports(&content, file_dir) {
        collect_with_imports(&imp, out, seen);
    }
}

/// Fichiers parcourus sous un dossier : dossiers cachés et `target/` exclus.
fn dir_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(e)  => e.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(e) => {
            eprintln!("ocaracs: impossible de lire '{}': {}", dir.display(), e);
            return Vec::new();
        }
    };
    entries.retain(|p| {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        !p.is_dir() || (!name.starts_with('.') && name != "target" && name != "node_modules")
    });
    entries.sort();
    entries
}

/// Mots des fichiers texte non `.oc` sous `dir` (templates, scripts…),
/// pour ne jamais renommer un nom qu'ils référencent.
fn collect_external_words(dir: &Path, out: &mut HashMap<String, PathBuf>) {
    const MAX_SIZE: u64 = 2 * 1024 * 1024;
    for path in dir_entries(dir) {
        if path.is_dir() {
            collect_external_words(&path, out);
        } else if path.extension().is_none_or(|e| e != "oc") && fs::metadata(&path).is_ok_and(|m| m.len() <= MAX_SIZE) {
            if let Ok(content) = fs::read_to_string(&path) {
                rename::external_words(&display_path(&path), &content, out);
            }
        }
    }
}

/// Tous les `.oc` sous `dir` ; avec `follow_imports`, les imports de chacun en plus.
fn collect_dir(dir: &Path, follow_imports: bool, out: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    for path in dir_entries(dir) {
        if path.is_dir() {
            collect_dir(&path, follow_imports, out, seen);
        } else if path.extension().is_some_and(|e| e == "oc") {
            if follow_imports {
                collect_with_imports(&path, out, seen);
            } else if let Ok(canonical) = path.canonicalize() {
                if seen.insert(canonical.clone()) { out.push(canonical); }
            }
        }
    }
}

fn analyzed_files(target: &Path) -> Vec<PathBuf> {
    let (mut out, mut seen) = (Vec::new(), HashSet::new());
    if target.is_dir() { collect_dir(target, true, &mut out, &mut seen); } else { collect_with_imports(target, &mut out, &mut seen); }
    out
}

/// Chemin affiché : relatif au dossier courant si possible.
fn display_path(path: &Path) -> PathBuf {
    std::env::current_dir().ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
}

fn check_all(files: &[PathBuf], cfg: &Config) -> usize {
    files.iter().filter_map(|f| fs::read_to_string(f).ok().map(|c| check::check_file(&display_path(f), &c, cfg))).sum()
}

// ─────────────────────────────────────────────────────────────────────────────
// --fix
// ─────────────────────────────────────────────────────────────────────────────

/// Renomme dans TOUT le projet (`root`), met en forme les fichiers analysés,
/// puis retourne ces fichiers (chemins après renommage éventuel). Avec
/// `dry_run`, rien n'est écrit : seul le compte rendu est affiché.
fn fix_all(analyzed: &[PathBuf], root: &Path, cfg: &Config, dry_run: bool) -> Vec<PathBuf> {
    let (mut project, mut seen) = (Vec::new(), HashSet::new());
    collect_dir(root, false, &mut project, &mut seen);
    project.extend(analyzed.iter().filter(|f| seen.insert((*f).clone())).cloned());
    let contents: Vec<(PathBuf, String)> = project.iter()
        .filter_map(|p| fs::read_to_string(p).ok().map(|c| (p.clone(), c)))
        .collect();
    let mut external = HashMap::new();
    collect_external_words(root, &mut external);
    let plan = rename::plan(&contents, &external, cfg);

    let (mut modified, mut replaced) = (0usize, 0usize);
    let mut result = Vec::new();
    for (path, content) in &contents {
        let (renamed, n) = if plan.is_empty() { (content.clone(), 0) } else { rename::apply(content, &plan) };
        replaced += n;
        let is_analyzed = analyzed.contains(path);
        let fixed = if is_analyzed { fix::fix_layout(&renamed, cfg) } else { renamed };
        let dest = plan.moved_path(path).unwrap_or_else(|| path.clone());
        if fixed != *content || dest != *path {
            if dry_run {
                eprintln!("ocaracs: fichier à modifier : {}", display_path(path).display());
            } else if let Err(e) = fs::write(&dest, &fixed) {
                eprintln!("ocaracs: impossible d'écrire '{}': {}", dest.display(), e);
                continue;
            }
            if dest != *path {
                if !dry_run { let _ = fs::remove_file(path); }
                eprintln!("ocaracs: fichier renommé : {} → {}", display_path(path).display(), display_path(&dest).display());
            }
            modified += 1;
        }
        if is_analyzed { result.push(dest); }
    }
    for (old, new) in plan.renames() { eprintln!("ocaracs: renommé : {} → {}", old, new); }
    for line in &plan.skipped { eprintln!("ocaracs: {}", line); }
    let verb = if dry_run { "à modifier" } else { "modifié(s)" };
    eprintln!("ocaracs --fix : {} fichier(s) {}, {} identifiant(s) renommé(s) ({} occurrence(s)).", modified, verb, plan.renames().len(), replaced);
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Point d'entrée
// ─────────────────────────────────────────────────────────────────────────────

fn print_help() {
    eprintln!("ocaracs — analyseur de style pour Ocara v1.0.0");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  ocaracs <fichier.oc>         Analyser un fichier (et ses imports)");
    eprintln!("  ocaracs <dossier>            Analyser tous les .oc d'un dossier");
    eprintln!("  ocaracs --fix <cible>        Corriger ce qui peut l'être, puis analyser");
    eprintln!("  ocaracs --fix --dry-run <c>  Afficher ce que --fix ferait, sans rien écrire");
    eprintln!();
    eprintln!("--fix corrige : indentation, lignes vides, espaces autour de '=' et en fin");
    eprintln!("de ligne, espace après '//', newline finale, nommage (déclaration et usages");
    eprintln!("dans tout le projet). Les lignes trop longues restent à corriger à la main.");
    eprintln!();
    eprintln!("Configuration:");
    eprintln!("  Fichier .ocaracs à la racine du projet (détecté automatiquement).");
    eprintln!("  Voir tools/ocaracs/README.md pour la liste des règles et options.");
    eprintln!();
    eprintln!("Codes de sortie:");
    eprintln!("  0  Aucun avertissement");
    eprintln!("  1  Avertissement(s) de style détecté(s)");
    eprintln!("  2  Erreur d'utilisation");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        std::process::exit(0);
    }
    let fix_mode = args.iter().any(|a| a == "--fix");
    let dry_run  = args.iter().any(|a| a == "--dry-run");
    let targets: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [target] = targets.as_slice() else {
        print_help();
        std::process::exit(2);
    };

    let target = PathBuf::from(target);
    if !target.exists() {
        eprintln!("ocaracs: cible introuvable : {}", target.display());
        std::process::exit(2);
    }

    let project_root = find_project_root(&target);
    let config       = load_config(&project_root);
    let mut files    = analyzed_files(&target);
    if fix_mode && dry_run {
        fix_all(&files, &project_root, &config, true);
        return;
    }
    if fix_mode {
        files = fix_all(&files, &project_root, &config, false);
    }
    let total = check_all(&files, &config);

    if total > 0 {
        eprintln!();
        eprintln!("{} avertissement(s) de style trouvé(s).", total);
        std::process::exit(1);
    }
}
