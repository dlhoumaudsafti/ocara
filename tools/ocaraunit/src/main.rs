// ─────────────────────────────────────────────────────────────────────────────
// ocaraunit — runner de tests unitaires pour Ocara
//
// Usage :
//   ocaraunit [options] [<dossier|fichier>]
//
// Par défaut exécute les tests du dossier `tests/` du dossier courant.
// Configuration : fichier .ocaraunit à la racine du projet.
//
// Convention de découverte (voir discovery.rs) :
//   - Fichiers  : *Test.oc
//   - Tests     : fonctions libres et méthodes publiques de classe dont le
//                 nom commence ou se termine par "test" (casse ignorée)
//
// Format de sortie des assertions (stdout du binaire compilé) :
//   PASS <message>
//   FAIL <message>
// ─────────────────────────────────────────────────────────────────────────────

mod cache;
mod cli;
mod config;
mod coverage;
mod discovery;
mod imports;
mod line_report;
mod lines;
mod process;
mod report;
mod runner;
mod scan;
#[cfg(test)]
mod tests;

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::exit,
    time::Duration,
};

use cache::{build_fingerprint, CACHE_DIR};
use cli::{detect_project_root, parse_args, Options};
use config::{collect_test_files, find_config_root, load_config, Config};
use discovery::extract_tests;
use report::{ansi, Ansi, Totals};
use runner::RunContext;

/// Code de sortie d'une erreur d'utilisation (option inconnue…).
const EXIT_USAGE: i32 = 2;

fn usage() {
    eprintln!("Usage: ocaraunit [options] [<dossier|fichier>]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --coverage                Analyse aussi la couverture (périmètre : racine du projet)");
    eprintln!("  --coverage=<chemin>       Couverture limitée à un dossier ou un fichier");
    eprintln!("  --line-coverage           Couverture par ligne (tests instrumentés, périmètre : racine du projet)");
    eprintln!("  --line-coverage=<chemin>  Couverture par ligne limitée à un dossier ou un fichier");
    eprintln!("  --exclude <chemin>        Exclut un dossier ou un fichier (tests et couverture), répétable");
    eprintln!("  --json <fichier>          Écrit résultats et couverture par ligne en JSON");
    eprintln!("  --src <dossier>           Racine du projet pour résoudre les imports");
    eprintln!("  --timeout <s>             Délai par fichier de test, compilation et exécution (défaut : {} s)",
              cli::DEFAULT_TIMEOUT_S);
    eprintln!("  --clear                   Supprimer le cache (seul) ou avant de lancer les tests");
    eprintln!("  --help, -h                Afficher cette aide");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <dossier>                 Exécuter tous les tests du dossier (défaut : tests/)");
    eprintln!("  <fichier>                 Exécuter un fichier de test spécifique");
}

/// Chemin absolu : un chemin relatif est résolu depuis le dossier de
/// lancement (compilation et tests s'exécutent depuis la racine du projet).
fn absolute(path: &Path, cwd: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| cwd.join(path))
}

fn find_ocara_bin(cwd: &Path) -> PathBuf {
    // 1. Variable d'environnement OCARA (relative au dossier de lancement)
    if let Ok(v) = std::env::var("OCARA") {
        let p = PathBuf::from(&v);
        if p.exists() { return absolute(&p, cwd); }
        if !v.contains('/') { return p; } // nom à chercher dans le PATH
    }
    // 2. Dans le même dossier que ocaraunit
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        let sibling = dir.join("ocara");
        if sibling.exists() { return sibling; }
    }
    // 3. Emplacements standards, puis relatifs au dossier courant (développement)
    for c in [
        "bin/ocara", "/usr/local/bin/ocara", "/usr/bin/ocara", "/opt/ocara/bin/ocara", "/opt/bin/ocara",
        "./target/release/ocara", "../target/release/ocara", "../../target/release/ocara",
    ] {
        let p = PathBuf::from(c);
        if p.exists() { return absolute(&p, cwd); }
    }
    // 4. Fallback : espérer qu'il est dans PATH
    PathBuf::from("ocara")
}

fn fail_usage(c: &Ansi, message: &str) -> ! {
    eprintln!("{}Erreur : {}{}", c.red, message, c.reset);
    eprintln!("Voir : ocaraunit --help");
    exit(EXIT_USAGE)
}

fn clear_cache(c: &Ansi, cache_dir: &Path) {
    if !cache_dir.exists() {
        println!("{}Aucun cache à supprimer{}", c.dim, c.reset);
        return;
    }
    match fs::remove_dir_all(cache_dir) {
        Ok(_) => println!("{}✓ Cache supprimé : {}{}", c.green, cache_dir.display(), c.reset),
        Err(e) => eprintln!("{}Erreur lors de la suppression du cache : {}{}", c.red, e, c.reset),
    }
}

/// Fichiers de test à exécuter et dossier qui les contient. Dossier absent
/// ou sans `*Test.oc` : erreur (jamais de succès sans test exécuté).
fn select_tests(c: &Ansi, target: &Path, cfg: &Config, cfg_root: &Path) -> (PathBuf, Vec<PathBuf>) {
    if target.is_file() {
        return (target.parent().unwrap_or(target).to_path_buf(), vec![target.to_path_buf()]);
    }
    if !target.is_dir() {
        eprintln!("{}Aucun dossier ou fichier de tests : {}{}", c.red, target.display(), c.reset);
        eprintln!("{}Créez le dossier ou spécifiez un chemin : ocaraunit <dossier|fichier>{}", c.dim, c.reset);
        exit(1);
    }
    let files = collect_test_files(target, &cfg.exclude, cfg_root);
    if files.is_empty() {
        eprintln!("{}Aucun fichier *Test.oc trouvé dans {}{}", c.red, target.display(), c.reset);
        exit(1);
    }
    (target.to_path_buf(), files)
}

fn main() {
    let c = ansi();
    let opts: Options = parse_args(std::env::args().skip(1)).unwrap_or_else(|e| fail_usage(&c, &e));
    if opts.help { usage(); exit(0); }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // `--clear` seul : purge le cache du projet courant et quitte.
    if opts.clear && opts.target.is_none() && opts.src.is_none() {
        clear_cache(&c, &detect_project_root(&cwd).join(CACHE_DIR));
        exit(0);
    }

    let target = absolute(Path::new(opts.target.as_deref().unwrap_or("tests")), &cwd);
    let project_root = match &opts.src {
        Some(dir) if !Path::new(dir).is_dir() => fail_usage(&c, &format!("--src : dossier introuvable : {}", dir)),
        Some(dir) => absolute(Path::new(dir), &cwd),
        None if opts.target.is_none() => cwd.clone(),
        None => detect_project_root(&target),
    };
    let cfg_root = find_config_root(&project_root).unwrap_or_else(|| project_root.clone());
    let mut cfg = load_config(&cfg_root);
    cfg.exclude.extend(opts.excludes.iter().cloned());
    let (scan_root, test_files) = select_tests(&c, &target, &cfg, &cfg_root);

    let cache_root = project_root.join(CACHE_DIR);
    if opts.clear { clear_cache(&c, &cache_root); }
    // Binaires instrumentés à part : alterner les deux modes ne recompile rien.
    let cache_dir = if opts.line_coverage { cache_root.join("lines") } else { cache_root };
    let _ = fs::create_dir_all(&cache_dir);

    let ocara = find_ocara_bin(&cwd);
    let ctx = RunContext {
        fingerprint: build_fingerprint(&ocara, &project_root),
        ocara,
        project_root: project_root.clone(),
        cache_dir,
        timeout: Duration::from_secs(opts.timeout_s),
        color: !c.reset.is_empty(),
        line_coverage: opts.line_coverage,
    };
    let mut hits = lines::Hits::default();
    let totals: Totals = report::run_all(&c, &ctx, &test_files, &mut hits);
    let scope = opts.coverage_target.as_deref().map(|t| absolute(Path::new(t), &cwd))
        .unwrap_or_else(|| project_root.clone());

    if opts.line_coverage {
        let (files, unanalyzed) = line_report::compute(&ctx, &scope, &scan_root, &test_files, &cfg, &cfg_root, &mut hits);
        line_report::print(&c, &files, &unanalyzed);
        if let Some(json) = &opts.json {
            let report = lines::Report { pass: totals.pass, fail: totals.fail, errors: totals.errors, files: &files, unanalyzed: &unanalyzed };
            if let Err(e) = fs::write(absolute(Path::new(json), &cwd), lines::to_json(&report)) {
                eprintln!("{}Erreur : écriture de {} : {}{}", c.red, json, e, c.reset);
            }
        }
    }

    if opts.coverage {
        let names: HashSet<String> = test_files.iter()
            .filter_map(|tf| fs::read_to_string(tf).ok())
            .flat_map(|src| extract_tests(&src))
            .map(|t| coverage::normalize(&t.name))
            .collect();
        report::print_coverage(&c, &scope, &scan_root, &test_files, &cfg, &cfg_root, &names);
    }

    exit(if totals.has_failure() { 1 } else { 0 });
}
