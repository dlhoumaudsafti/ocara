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
mod members;
mod naming;
mod project;
mod rename;
mod rewrite;
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

/// Fichiers analysés et fichiers illisibles (absents ou pas en UTF-8).
#[derive(Default)]
struct Collected {
    files:      Vec<PathBuf>,
    seen:       HashSet<PathBuf>,
    unreadable: usize,
}

/// Lit `path` ; une erreur (UTF-8 invalide…) est signalée et comptée.
fn read_source(path: &Path, unreadable: &mut usize) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(content) => Some(content),
        Err(e) => {
            eprintln!("ocaracs: impossible de lire '{}': {}", display_path(path).display(), e);
            *unreadable += 1;
            None
        }
    }
}

/// Ajoute `path` (canonique) et, récursivement, les fichiers qu'il importe,
/// résolus depuis `root` (dossier du fichier d'entrée, comme le compilateur).
fn collect_with_imports(path: &Path, root: &Path, out: &mut Collected, report_missing: bool) {
    let Ok(canonical) = path.canonicalize() else {
        if report_missing { eprintln!("ocaracs: import introuvable : {}", display_path(path).display()); }
        return;
    };
    if !out.seen.insert(canonical.clone()) { return; }
    let Some(content) = read_source(&canonical, &mut out.unreadable) else { return };
    out.files.push(canonical.clone());
    let file_dir = canonical.parent().unwrap_or(Path::new("."));
    for imp in project::referenced_files(&content, file_dir, root) {
        collect_with_imports(&imp, root, out, report_missing);
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

/// Tous les `.oc` sous `dir` ; avec `root`, les imports de chacun en plus.
fn collect_dir(dir: &Path, root: Option<&Path>, out: &mut Collected) {
    for path in dir_entries(dir) {
        if path.is_dir() {
            collect_dir(&path, root, out);
        } else if path.extension().is_some_and(|e| e == "oc") {
            match root {
                Some(root) => collect_with_imports(&path, root, out, false),
                None => if let Ok(canonical) = path.canonicalize() {
                    if out.seen.insert(canonical.clone()) { out.files.push(canonical); }
                },
            }
        }
    }
}

/// Fichiers analysés pour `target` : un dossier (tous ses `.oc` et leurs
/// imports, résolus depuis ce dossier) ou un fichier d'entrée et ses imports.
fn analyzed_files(target: &Path) -> Collected {
    let mut out = Collected::default();
    if target.is_dir() {
        let root = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());
        collect_dir(target, Some(&root), &mut out);
    } else {
        let root = target.canonicalize().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."));
        collect_with_imports(target, &root, &mut out, true);
    }
    out
}

/// Chemin affiché : relatif au dossier courant si possible.
fn display_path(path: &Path) -> PathBuf {
    std::env::current_dir().ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
}

/// (avertissements, fichiers illisibles).
fn check_all(files: &[PathBuf], cfg: &Config) -> (usize, usize) {
    let mut unreadable = 0;
    let warnings = files.iter()
        .filter_map(|f| read_source(f, &mut unreadable).map(|c| check::check_file(&display_path(f), &c, cfg)))
        .sum();
    (warnings, unreadable)
}

// ─────────────────────────────────────────────────────────────────────────────
// --fix
// ─────────────────────────────────────────────────────────────────────────────

/// Renomme dans TOUT le projet (`root`), met en forme les fichiers analysés,
/// puis retourne ces fichiers (chemins après renommage éventuel). Avec
/// `dry_run`, rien n'est écrit : seul le compte rendu est affiché.
fn fix_all(analyzed: &[PathBuf], root: &Path, cfg: &Config, dry_run: bool) -> Vec<PathBuf> {
    let mut project = Collected::default();
    collect_dir(root, None, &mut project);
    let mut files = project.files;
    files.extend(analyzed.iter().filter(|f| project.seen.insert((*f).clone())).cloned());
    let contents: Vec<(PathBuf, String)> = files.iter()
        .filter_map(|p| read_source(p, &mut project.unreadable).map(|c| (p.clone(), c)))
        .collect();
    let mut external = HashMap::new();
    collect_external_words(root, &mut external);
    // Un fichier illisible peut contenir des usages : aucun renommage.
    let plan = if project.unreadable > 0 {
        eprintln!("ocaracs: fichier(s) illisible(s) dans le projet : renommages désactivés");
        rename::Plan::default()
    } else {
        rename::plan(&contents, &external, cfg)
    };

    let (mut modified, mut replaced) = (0usize, 0usize);
    let mut result = Vec::new();
    for (path, content) in &contents {
        let (renamed, n) = if plan.is_empty() { (content.clone(), 0) } else { rename::apply(path, content, &plan) };
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
    eprintln!("  2  Erreur d'utilisation, ou fichier .oc illisible (UTF-8 invalide)");
}

/// Options reconnues (en plus de `--help`/`-h`).
const OPTIONS: [&str; 2] = ["--fix", "--dry-run"];

fn usage_error(msg: &str) -> ! {
    eprintln!("ocaracs: {}", msg);
    eprintln!("Voir 'ocaracs --help'.");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        std::process::exit(0);
    }
    if let Some(unknown) = args.iter().find(|a| a.starts_with('-') && !OPTIONS.contains(&a.as_str())) {
        usage_error(&format!("option inconnue : {}", unknown));
    }
    let fix_mode = args.iter().any(|a| a == "--fix");
    let dry_run  = args.iter().any(|a| a == "--dry-run");
    if dry_run && !fix_mode { usage_error("--dry-run s'utilise avec --fix"); }
    let targets: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    let [target] = targets.as_slice() else {
        print_help();
        std::process::exit(2);
    };

    let target = PathBuf::from(target);
    if !target.exists() {
        usage_error(&format!("cible introuvable : {}", target.display()));
    }

    let project_root = find_project_root(&target);
    let config       = load_config(&project_root);
    let collected    = analyzed_files(&target);
    let mut files    = collected.files;
    if fix_mode && dry_run {
        fix_all(&files, &project_root, &config, true);
        std::process::exit(if collected.unreadable > 0 { 2 } else { 0 });
    }
    if fix_mode {
        files = fix_all(&files, &project_root, &config, false);
    }
    let (total, unreadable) = check_all(&files, &config);

    if total > 0 {
        eprintln!();
        eprintln!("{} avertissement(s) de style trouvé(s).", total);
    }
    if unreadable + collected.unreadable > 0 {
        eprintln!("ocaracs: {} fichier(s) illisible(s) (UTF-8 invalide ?) non analysé(s).", unreadable + collected.unreadable);
        std::process::exit(2);
    }
    if total > 0 { std::process::exit(1); }
}
