// ─────────────────────────────────────────────────────────────────────────────
// ocaraunit — runner de tests unitaires pour Ocara
//
// Usage :
//   ocaraunit [--coverage] [<dossier>]
//
// Par défaut scanne le dossier courant.
// Configuration : fichier .ocaraunit à la racine du projet.
//
// Convention de découverte :
//   - Fichiers    : *Test.oc
//   - Script sans classe : toute fonction dont le nom se termine par "Test"
//   - Classe       : toute méthode publique dont le nom se termine par "Test"
//
// Format de sortie des assertions (stdout du binaire compilé) :
//   PASS <message>
//   FAIL <message>
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    io::IsTerminal,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

// ─────────────────────────────────────────────────────────────────────────────
// Couleurs ANSI
// ─────────────────────────────────────────────────────────────────────────────

struct Ansi {
    green:  &'static str,
    yellow: &'static str,
    red:    &'static str,
    bold:   &'static str,
    reset:  &'static str,
    dim:    &'static str,
}

fn ansi() -> Ansi {
    let color = std::io::stdout().is_terminal()
        && std::env::var("NO_COLOR").is_err();
    if color {
        Ansi {
            green:  "\x1b[0;32m",
            yellow: "\x1b[0;33m",
            red:    "\x1b[0;31m",
            bold:   "\x1b[1m",
            reset:  "\x1b[0m",
            dim:    "\x1b[2m",
        }
    } else {
        Ansi { green: "", yellow: "", red: "", bold: "", reset: "", dim: "" }
    }
}

fn coverage_color<'a>(pct: f64, c: &'a Ansi) -> &'a str {
    if pct >= 100.0 { c.green }
    else if pct >= 75.0 { c.yellow }
    else { c.red }
}

// ─────────────────────────────────────────────────────────────────────────────
// Configuration (.ocaraunit)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct Config {
    /// Répertoires ou fichiers exclus de l'analyse de couverture
    exclude: Vec<String>,
}

fn find_config_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(".ocaraunit");
        if candidate.exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn load_config(root: &Path) -> Config {
    let path = root.join(".ocaraunit");
    let content = match fs::read_to_string(&path) {
        Ok(c)  => c,
        Err(_) => return Config::default(),
    };
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

fn is_excluded(path: &Path, root: &Path, excludes: &[String]) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel_str = rel.to_string_lossy();
    excludes.iter().any(|ex| {
        let ex_path = Path::new(ex.as_str());
        rel.starts_with(ex_path) || rel_str.contains(ex.as_str())
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Recherche des fichiers .oc
// ─────────────────────────────────────────────────────────────────────────────

fn collect_oc_files(dir: &Path, excludes: &[String], root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let rd = match fs::read_dir(dir) {
        Ok(r)  => r,
        Err(_) => return files,
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if is_excluded(&path, root, excludes) { continue; }
        if path.is_dir() {
            files.extend(collect_oc_files(&path, excludes, root));
        } else if path.extension().and_then(|e| e.to_str()) == Some("oc") {
            files.push(path);
        }
    }
    files
}

fn collect_test_files(dir: &Path, excludes: &[String], root: &Path) -> Vec<PathBuf> {
    collect_oc_files(dir, excludes, root)
        .into_iter()
        .filter(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.ends_with("Test"))
                .unwrap_or(false)
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Analyse statique — extraction des noms de tests dans un fichier .oc
// ─────────────────────────────────────────────────────────────────────────────

/// Détecte si le fichier contient une déclaration de classe.
fn file_has_class(source: &str) -> bool {
    source.lines().any(|l| {
        let t = l.trim();
        t.starts_with("class ") || t == "class"
    })
}

/// Extrait le nom de la première classe déclarée dans le fichier.
fn extract_class_name(source: &str) -> Option<String> {
    for line in source.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("class ") {
            let name = rest.split_whitespace().next().unwrap_or("").trim_matches('{').trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Génère un `function main()` qui appelle tous les tests du fichier.
/// - Script : appelle directement chaque fonction `fooTest()`
/// - Classe  : instancie la classe (sans arguments) puis appelle chaque méthode
fn generate_main(source: &str, test_names: &[String]) -> String {
    if test_names.is_empty() {
        return "\nfunction main(): int {\n    return 0\n}\n".to_string();
    }
    let mut code = String::from("\nfunction main(): int {\n");
    if file_has_class(source) {
        if let Some(class_name) = extract_class_name(source) {
            code.push_str(&format!("    scoped _t:{} = use {}()\n", class_name, class_name));
            for name in test_names {
                code.push_str(&format!("    _t.{}()\n", name));
            }
        }
    } else {
        for name in test_names {
            code.push_str(&format!("    {}()\n", name));
        }
    }
    code.push_str("    return 0\n}\n");
    code
}

/// Extrait les noms de fonctions/méthodes de test selon la convention ocaraunit.
///
/// - Script sans classe : `function <name>Test(...)`
/// - Classe : `public method <name>Test(...)`
fn extract_test_names(source: &str) -> Vec<String> {
    let has_class = file_has_class(source);
    let mut names = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if has_class {
            // Cherche : public method <ident>Test (
            if let Some(rest) = trimmed.strip_prefix("public method ") {
                let ident = rest
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim();
                if ident.ends_with("Test") {
                    names.push(ident.to_string());
                }
            }
        } else {
            // Cherche : function <ident>Test (
            if let Some(rest) = trimmed.strip_prefix("function ") {
                let ident = rest
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim();
                if ident.ends_with("Test") {
                    names.push(ident.to_string());
                }
            }
        }
    }
    names
}

// ─────────────────────────────────────────────────────────────────────────────
// Résultats d'un test
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct AssertResult {
    passed: bool,
    message: String,
}

#[derive(Debug)]
struct TestResult {
    asserts: Vec<AssertResult>,
    /// true si la compilation ou l'exécution a échoué
    error:   Option<String>,
    /// Temps de compilation en millisecondes
    compile_time_ms: u128,
    /// Temps d'exécution en millisecondes
    run_time_ms: u128,
}

impl TestResult {
    fn pass_count(&self) -> usize {
        self.asserts.iter().filter(|a| a.passed).count()
    }
    fn fail_count(&self) -> usize {
        self.asserts.iter().filter(|a| !a.passed).count()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Exécution d'un test
// ─────────────────────────────────────────────────────────────────────────────

/// Calcule un hash rapide du contenu d'un fichier pour le cache.
fn hash_file_content(path: &Path) -> Option<u64> {
    let content = fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    Some(hasher.finish())
}

fn find_ocara_bin() -> PathBuf {
    // 1. Variable d'environnement OCARA
    if let Ok(v) = std::env::var("OCARA") {
        let p = PathBuf::from(v);
        if p.exists() { return p; }
    }
    
    // 2. Dans le même dossier que ocaraunit (le plus évident)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("ocara");
            if sibling.exists() { return sibling; }
        }
    }
    
    // 3. Emplacements standards Unix/Linux
    for system_path in [
        "bin/ocara",
        "/usr/local/bin/ocara",
        "/usr/bin/ocara",
        "/opt/ocara/bin/ocara",
        "/opt/bin/ocara",
    ] {
        let p = PathBuf::from(system_path);
        if p.exists() { return p; }
    }
    
    // 4. Relatif au dossier courant (pour développement)
    let candidates = [
        "./target/release/ocara",
        "../target/release/ocara",
        "../../target/release/ocara",
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() { return p; }
    }
    
    // 5. Fallback : espérer qu'il est dans PATH
    PathBuf::from("ocara")
}

/// Compile et exécute un fichier de test Ocara.
///
/// Processus :
/// 1. Lit le fichier source et extrait tous les noms de tests (*Test)
/// 2. Génère une fonction main() qui appelle tous les tests
/// 3. Compile avec `ocara --src <project_root>` pour résoudre les imports
/// 4. Met en cache le binaire (hash du fichier source)
/// 5. Exécute le binaire et parse la sortie (lignes PASS/FAIL)
///
/// Le cache (`.ocaraunit_cache/`) évite de recompiler si le fichier n'a pas changé.
/// Utilisez `--clear` pour forcer la recompilation après une mise à jour du compilateur.
fn run_test_file(
    ocara: &Path,
    src: &Path,
    cache_dir: &Path,
    project_root: &Path,
) -> (Vec<TestResult>, Option<String>) {
    let stem = src.file_stem().unwrap_or_default().to_string_lossy();
    
    // Calculer le hash du fichier source pour le cache
    let source_hash = match hash_file_content(src) {
        Some(h) => h,
        None => return (vec![], Some("impossible de lire le fichier source".to_string())),
    };
    
    let bin = cache_dir.join(format!("{}-{:x}", stem, source_hash));
    let mut compile_time_ms = 0u128;
    let mut used_cache = false;
    
    // Vérifier si un binaire caché existe
    if !bin.exists() {
        // Le compilateur résout les imports relatifs au répertoire du fichier source.
        // On copie le fichier test à la racine du projet et on y injecte un main()
        // qui appelle toutes les fonctions/méthodes *Test.
        let tmp_src = project_root.join(format!("__ocaraunit__{}.oc", stem));
        let source = match fs::read_to_string(src) {
            Ok(s)  => s,
            Err(e) => return (vec![], Some(format!("impossible de lire le fichier : {}", e))),
        };
        let test_names = extract_test_names(&source);
        let main_code  = generate_main(&source, &test_names);
        let _ = fs::write(&tmp_src, format!("{}\n{}", source, main_code));

        // Compilation depuis la racine du projet
        let compile_start = Instant::now();
        let mut cmd = Command::new(ocara);
        cmd.args([tmp_src.as_os_str(), std::ffi::OsStr::new("-o"), bin.as_os_str()])
            .args([std::ffi::OsStr::new("--src"), project_root.as_os_str()])
            .current_dir(project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let compile = cmd.output();
        compile_time_ms = compile_start.elapsed().as_millis();

        let _ = fs::remove_file(&tmp_src); // nettoyage immédiat

        let compile_out = match compile {
            Err(e) => return (vec![], Some(format!("impossible de lancer ocara : {}", e))),
            Ok(o)  => o,
        };

        if !compile_out.status.success() {
            let stderr = String::from_utf8_lossy(&compile_out.stderr).to_string();
            // Afficher les 15 premières lignes d'erreur au lieu d'une seule
            let error_lines: Vec<&str> = stderr.lines().take(15).collect();
            return (vec![], Some(error_lines.join("\n")));
        }
    } else {
        used_cache = true;
    }

    // Exécution
    let run_start = Instant::now();
    let run_out = Command::new(&bin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let run_time_ms = run_start.elapsed().as_millis();

    let run_out = match run_out {
        Err(e) => return (vec![], Some(format!("impossible d'exécuter le binaire : {}", e))),
        Ok(o)  => o,
    };

    // Parse stdout : lignes PASS ... / FAIL ...
    let stdout = String::from_utf8_lossy(&run_out.stdout);
    let mut all_asserts: Vec<AssertResult> = Vec::new();
    for line in stdout.lines() {
        if let Some(msg) = line.strip_prefix("PASS ") {
            all_asserts.push(AssertResult { passed: true,  message: msg.to_string() });
        } else if let Some(msg) = line.strip_prefix("FAIL ") {
            all_asserts.push(AssertResult { passed: false, message: msg.to_string() });
        }
    }

    // On regroupe sous un TestResult unique par fichier (pas d'isolation par fonction)
    let _name = stem.to_string();
    let error = if !run_out.status.success() && all_asserts.is_empty() {
        let stderr = String::from_utf8_lossy(&run_out.stderr).to_string();
        // Afficher les 10 premières lignes d'erreur
        let error_lines: Vec<&str> = stderr.lines().take(10).collect();
        Some(error_lines.join("\n"))
    } else {
        None
    };

    (vec![TestResult { 
        asserts: all_asserts, 
        error,
        compile_time_ms: if used_cache { 0 } else { compile_time_ms },
        run_time_ms,
    }], None)
}

// ─────────────────────────────────────────────────────────────────────────────
// Analyse de couverture
// ─────────────────────────────────────────────────────────────────────────────

/// Couverture d'un fichier source : ratio fonctions avec *Test sur total fonctions.
#[derive(Debug)]
struct FileCoverage {
    path:           PathBuf,
    total_funcs:    usize,
    covered_funcs:  usize,
    /// Fonctions non couvertes : (nom, numéro de ligne 1-basé)
    uncovered:      Vec<(String, usize)>,
}

impl FileCoverage {
    fn percent(&self) -> f64 {
        if self.total_funcs == 0 { return 100.0; }
        (self.covered_funcs as f64 / self.total_funcs as f64) * 100.0
    }
}

/// Extrait tous les noms de fonctions/méthodes avec leur numéro de ligne.
fn extract_all_function_names(source: &str) -> Vec<(String, usize)> {
    let has_class = file_has_class(source);
    let mut names = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let t = line.trim();
        let lineno = line_idx + 1;
        if has_class {
            // public [static] method <name>(  /  protected method …  /  private method …
            for vis in [
                "public static method ",
                "protected static method ",
                "private static method ",
                "public method ",
                "protected method ",
                "private method ",
            ] {
                if let Some(rest) = t.strip_prefix(vis) {
                    let ident = rest.split('(').next().unwrap_or("").trim();
                    if !ident.is_empty() && ident != "init" {
                        names.push((ident.to_string(), lineno));
                    }
                    break;
                }
            }
        } else {
            // function <name>(
            if let Some(rest) = t.strip_prefix("function ") {
                let ident = rest.split('(').next().unwrap_or("").trim();
                if !ident.is_empty() {
                    names.push((ident.to_string(), lineno));
                }
            }
        }
    }
    names
}

/// Calcule la couverture d'un fichier source non-test.
/// "couverte" = il existe un fichier *Test.oc qui importe ce fichier
/// ET qui contient un test dont le nom correspond au schéma <funcName>Test.
fn compute_coverage(
    src_file: &Path,
    all_funcs: &[(String, usize)],
    test_names_global: &HashSet<String>,
) -> FileCoverage {
    let mut covered = 0usize;
    let mut uncovered = Vec::new();
    for (name, lineno) in all_funcs {
        let expected = format!("{}Test", name);
        if test_names_global.contains(&expected) {
            covered += 1;
        } else {
            uncovered.push((name.clone(), *lineno));
        }
    }
    FileCoverage {
        path:          src_file.to_path_buf(),
        total_funcs:   all_funcs.len(),
        covered_funcs: covered,
        uncovered,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Affichage
// ─────────────────────────────────────────────────────────────────────────────

fn rel_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.strip_prefix(&cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn print_separator(c: &Ansi) {
    println!("{}══════════════════════════════════════════════{}", c.dim, c.reset);
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

fn usage() {
    eprintln!("Usage: ocaraunit [options] [<dossier|fichier>]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --coverage [<dossier|fichier>]  Analyse la couverture (défaut : tout le projet)");
    eprintln!("  --src <dossier>                 Dossier racine du projet pour résoudre les imports");
    eprintln!("  --clear                         Supprimer le cache (seul) ou avant de lancer les tests");
    eprintln!("  --help, -h                      Afficher cette aide");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <dossier>                  Exécuter tous les tests du dossier (défaut : tests/)");
    eprintln!("  <fichier>                  Exécuter un fichier de test spécifique");
    eprintln!();
    eprintln!("Convention :");
    eprintln!("  Les fichiers de test doivent être dans un dossier tests/ à la racine du projet.");
    eprintln!("  Nommage : pour foo/Bar.oc le test doit être tests/BarTest.oc");
}

fn main() {
    let c = ansi();

    let mut args = std::env::args().skip(1).peekable();
    let mut coverage_mode = false;
    let mut coverage_source_arg: Option<String> = None;
    let mut test_dir_arg: Option<String> = None;
    let mut src_arg: Option<String> = None;
    let mut clear_cache = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--coverage" => {
                coverage_mode = true;
                // Argument optionnel suivant = dossier source pour couverture
                if let Some(next) = args.peek() {
                    if !next.starts_with('-') {
                        coverage_source_arg = args.next();
                    }
                }
            }
            "--src" => {
                if let Some(dir) = args.next() {
                    src_arg = Some(dir);
                } else {
                    eprintln!("{}Erreur: --src nécessite un argument{}", c.red, c.reset);
                    std::process::exit(1);
                }
            }
            "--clear" => {
                clear_cache = true;
            }
            "--help" | "-h" => { usage(); std::process::exit(0); }
            other => {
                // Si ce n'est pas une option, c'est le dossier de tests
                if !other.starts_with('-') && test_dir_arg.is_none() {
                    test_dir_arg = Some(other.to_string());
                }
            }
        }
    }

    // Déterminer le dossier ou fichier de tests
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    
    // Si --clear est utilisé seul (sans tests), purger et quitter
    if clear_cache && test_dir_arg.is_none() {
        let config_root = find_config_root(&cwd).unwrap_or_else(|| cwd.clone());
        let cache_dir = config_root.join(".ocaraunit_cache");
        if cache_dir.exists() {
            match fs::remove_dir_all(&cache_dir) {
                Ok(_) => println!("{}✓ Cache supprimé : {}{}", c.green, cache_dir.display(), c.reset),
                Err(e) => eprintln!("{}Erreur lors de la suppression du cache : {}{}", c.red, e, c.reset),
            }
        } else {
            println!("{}Aucun cache à supprimer{}", c.dim, c.reset);
        }
        std::process::exit(0);
    }
    
    // Garder une copie pour project_root
    let test_dir_arg_clone = test_dir_arg.clone();
    
    // Vérifier si l'argument est un fichier spécifique
    let (_scan_root, test_files) = if let Some(path_arg) = test_dir_arg {
        let path = PathBuf::from(&path_arg);
        if path.is_file() {
            // Fichier spécifique fourni
            if !path.exists() {
                println!("{}Fichier de test introuvable : {}{}",
                    c.red, path.display(), c.reset);
                std::process::exit(1);
            }
            let dir = path.parent().unwrap_or(&cwd).to_path_buf();
            let scan = fs::canonicalize(&dir).unwrap_or(dir.clone());
            (scan.clone(), vec![path])
        } else if path.is_dir() {
            // Dossier fourni
            if !path.exists() {
                println!("{}Aucun dossier de tests trouvé : {}{}",
                    c.yellow, path.display(), c.reset);
                println!("{}Créez le dossier ou spécifiez un chemin : ocaraunit <dossier|fichier>{}", c.dim, c.reset);
                std::process::exit(0);
            }
            let scan = fs::canonicalize(&path).unwrap_or(path.clone());
            let cfg_root = find_config_root(&cwd).unwrap_or_else(|| cwd.clone());
            let cfg = load_config(&cfg_root);
            let files = collect_test_files(&scan, &cfg.exclude, &cfg_root);
            (scan, files)
        } else {
            println!("{}Chemin invalide : {}{}",
                c.red, path.display(), c.reset);
            std::process::exit(1);
        }
    } else {
        // Par défaut : chercher ./tests/
        let tests_dir = cwd.join("tests");
        if !tests_dir.exists() {
            println!("{}Aucun dossier de tests trouvé : {}{}",
                c.yellow, tests_dir.display(), c.reset);
            println!("{}Créez le dossier ou spécifiez un chemin : ocaraunit <dossier|fichier>{}", c.dim, c.reset);
            std::process::exit(0);
        }
        let scan = fs::canonicalize(&tests_dir).unwrap_or(tests_dir.clone());
        let cfg_root = find_config_root(&cwd).unwrap_or_else(|| cwd.clone());
        let cfg = load_config(&cfg_root);
        let files = collect_test_files(&scan, &cfg.exclude, &cfg_root);
        (scan, files)
    };

    // Déterminer si --coverage pointe vers un fichier unique pour limiter la couverture
    let coverage_single_file: Option<PathBuf> = if let Some(ref cov_arg) = coverage_source_arg {
        let p = PathBuf::from(cov_arg);
        let abs_p = fs::canonicalize(&p).unwrap_or(p.clone());
        if abs_p.is_file() {
            Some(abs_p)
        } else {
            None
        }
    } else {
        None
    };

    // Dossier source pour la couverture (argument ou cwd)
    let coverage_source = if let Some(ref single_file) = coverage_single_file {
        // Si --coverage est un fichier, utiliser son dossier parent comme racine
        single_file.parent().unwrap_or(&cwd).to_path_buf()
    } else {
        fs::canonicalize(
            PathBuf::from(coverage_source_arg.unwrap_or_else(|| ".".to_string()))
        ).unwrap_or_else(|_| cwd.clone())
    };
    
    // Déterminer le dossier racine du projet pour résoudre les imports
    let project_root = if let Some(proj_dir) = src_arg {
        // Option --src fournie explicitement
        let p = PathBuf::from(proj_dir);
        fs::canonicalize(&p).unwrap_or(p)
    } else if let Some(ref test_path_arg) = test_dir_arg_clone.as_ref() {
        // Détecter automatiquement depuis le chemin fourni
        let test_path = PathBuf::from(test_path_arg);
        let abs_path = if test_path.is_absolute() {
            test_path.clone()
        } else {
            cwd.join(&test_path)
        };
        
        // Si le chemin se termine par "tests" ou contient "tests/", le projet est le parent
        if abs_path.ends_with("tests") {
            abs_path.parent().unwrap_or(&cwd).to_path_buf()
        } else if abs_path.components().any(|c| c.as_os_str() == "tests") {
            // Remonter jusqu'au parent de "tests"
            let mut proj = abs_path.clone();
            while let Some(parent) = proj.parent() {
                if parent.join("tests").exists() {
                    proj = parent.to_path_buf();
                    break;
                }
                proj = parent.to_path_buf();
            }
            proj
        } else {
            // Le chemin fourni est probablement le projet lui-même
            abs_path
        }
    } else {
        // Par défaut : dossier courant
        cwd.clone()
    };

    // Chercher la racine de config en remontant depuis cwd
    let config_root = find_config_root(&cwd).unwrap_or_else(|| cwd.clone());
    let cfg = load_config(&config_root);

    let ocara = find_ocara_bin();

    // Répertoire de cache pour les binaires compilés
    let cache_dir = config_root.join(".ocaraunit_cache");
    
    // Supprimer le cache si --clear est activé
    if clear_cache && cache_dir.exists() {
        match fs::remove_dir_all(&cache_dir) {
            Ok(_) => println!("{}✓ Cache supprimé{}", c.green, c.reset),
            Err(e) => eprintln!("{}Erreur lors de la suppression du cache : {}{}", c.red, e, c.reset),
        }
    }
    
    let _ = fs::create_dir_all(&cache_dir);

    // ── Collecte des noms de tests (pour couverture) ─────────────────────────
    let mut global_test_names: HashSet<String> = HashSet::new();
    for tf in &test_files {
        if let Ok(src) = fs::read_to_string(tf) {
            for name in extract_test_names(&src) {
                global_test_names.insert(name);
            }
        }
    }

    // ── Exécution des tests ──────────────────────────────────────────────────
    let mut total_pass = 0usize;
    let mut total_fail = 0usize;
    let mut total_errors = 0usize;
    let mut has_failure = false;
    let mut total_compile_time_ms = 0u128;
    let mut total_run_time_ms = 0u128;

    if test_files.is_empty() {
        print_separator(&c);
        println!("{} Tests ocaraunit{}", c.bold, c.reset);
        print_separator(&c);
        println!("{}Aucun fichier *Test.oc trouvé dans tests/{}", c.yellow, c.reset);
    } else {
        print_separator(&c);
        println!("{} Tests ocaraunit{}", c.bold, c.reset);
        print_separator(&c);

    for tf in &test_files {
        let rel = rel_path(tf);
        println!();
        println!("{}{}{}:", c.bold, rel, c.reset);

        let (results, compile_err) = run_test_file(&ocara, tf, &cache_dir, &project_root);

        if let Some(err) = compile_err {
            println!("  {}ERREUR compilation :{}", c.red, c.reset);
            for line in err.lines() {
                println!("  {}", line);
            }
            total_errors += 1;
            has_failure = true;
            continue;
        }

        for res in &results {
            if let Some(ref err) = res.error {
                println!("  {}ERREUR exécution :{}", c.red, c.reset);
                for line in err.lines() {
                    println!("  {}", line);
                }
                total_errors += 1;
                has_failure = true;
                continue;
            }
            
            total_compile_time_ms += res.compile_time_ms;
            total_run_time_ms += res.run_time_ms;

            if res.asserts.is_empty() {
                println!("  {}(aucune assertion){}", c.dim, c.reset);
                continue;
            }

            for a in &res.asserts {
                if a.passed {
                    println!("  {}PASS{} {}", c.green, c.reset, a.message);
                    total_pass += 1;
                } else {
                    println!("  {}FAIL{} {}", c.red, c.reset, a.message);
                    total_fail += 1;
                    has_failure = true;
                }
            }

            let p = res.pass_count();
            let f = res.fail_count();
            let col = if f == 0 { c.green } else { c.red };
            let time_info = if res.compile_time_ms > 0 {
                format!(" {}(compile: {}ms, run: {}ms){}",
                    c.dim, res.compile_time_ms, res.run_time_ms, c.reset)
            } else {
                format!(" {}(cached, run: {}ms){}", c.dim, res.run_time_ms, c.reset)
            };
            println!("  {}{} PASS  {} FAIL{}{}", col, p, f, c.reset, time_info);
        }
    }

    println!();
    print_separator(&c);
    let global_col = if has_failure { c.red } else { c.green };
    let total_time_s = (total_compile_time_ms + total_run_time_ms) as f64 / 1000.0;
    println!("{}Résultat global : {} PASS  {} FAIL  {} ERREUR(S){}",
        global_col, total_pass, total_fail, total_errors, c.reset);
    if total_compile_time_ms > 0 || total_run_time_ms > 0 {
        println!("{}Temps total : {:.2}s (compile: {:.2}s, run: {:.2}s){}",
            c.dim,
            total_time_s,
            total_compile_time_ms as f64 / 1000.0,
            total_run_time_ms as f64 / 1000.0,
            c.reset);
    }
    print_separator(&c);
    } // fin else test_files non vide

    // ── Analyse de couverture ────────────────────────────────────────────────
    if coverage_mode {
        println!();
        print_separator(&c);
        println!("{} Couverture de tests{}", c.bold, c.reset);
        print_separator(&c);
        println!();

        // Si --coverage pointe vers un fichier unique, limiter la couverture à ce fichier
        let source_files: Vec<PathBuf> = if let Some(ref single_file) = coverage_single_file {
            // Couverture limitée au fichier spécifié uniquement
            vec![single_file.clone()]
        } else {
            // Comportement par défaut : tous les fichiers du dossier
            let all_oc = collect_oc_files(&coverage_source, &cfg.exclude, &config_root);
            // Exclure les fichiers *Test.oc eux-mêmes et le dossier tests/
            let test_file_set: HashSet<PathBuf> = test_files.iter().cloned().collect();
            all_oc
                .into_iter()
                .filter(|p| !test_file_set.contains(p))
                .collect()
        };

        let mut total_funcs_all = 0usize;
        let mut covered_funcs_all = 0usize;
        let mut coverages: Vec<FileCoverage> = Vec::new();

        for sf in &source_files {
            if let Ok(src) = fs::read_to_string(sf) {
                let funcs = extract_all_function_names(&src);
                let cov = compute_coverage(sf, &funcs, &global_test_names);
                total_funcs_all   += cov.total_funcs;
                covered_funcs_all += cov.covered_funcs;
                coverages.push(cov);
            }
        }

        // Affichage par fichier
        let max_path_len = coverages.iter()
            .map(|c| rel_path(&c.path).len())
            .max()
            .unwrap_or(0)
            .max(20);

        for cov in &coverages {
            let pct = cov.percent();
            let col = coverage_color(pct, &c);
            let path_str = rel_path(&cov.path);
            // Barre de progression ASCII (20 chars)
            let filled = ((pct / 100.0) * 20.0).round() as usize;
            let bar: String = format!("[{}{}]",
                "█".repeat(filled),
                "░".repeat(20usize.saturating_sub(filled)),
            );
            println!("  {:<width$}  {}{:>6.1}%  {}  {}/{} fonctions{}",
                path_str,
                col, pct, bar,
                cov.covered_funcs, cov.total_funcs,
                c.reset,
                width = max_path_len,
            );
            // Afficher les fonctions non couvertes avec leur numéro de ligne
            for (fname, lineno) in &cov.uncovered {
                println!("    {}{}:{}:{}: non couverte — manque {}Test{}",
                    c.red, rel_path(&cov.path), lineno, 1, fname, c.reset);
            }
        }

        if coverages.is_empty() {
            println!("  {}Aucun fichier source trouvé.{}", c.dim, c.reset);
        }

        println!();
        print_separator(&c);
        let global_pct = if total_funcs_all == 0 {
            100.0f64
        } else {
            (covered_funcs_all as f64 / total_funcs_all as f64) * 100.0
        };
        let global_cov_col = coverage_color(global_pct, &c);
        println!("{} Couverture globale : {}{:.1}%{} ({}/{} fonctions){}",
            c.bold,
            global_cov_col, global_pct, c.bold,
            covered_funcs_all, total_funcs_all,
            c.reset,
        );
        print_separator(&c);
    }

    // Note: on ne nettoie PAS le cache pour qu'il persiste entre exécutions
    
    std::process::exit(if has_failure { 1 } else { 0 });
}
