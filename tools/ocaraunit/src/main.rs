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
    collections::HashSet,
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::{Command, Stdio},
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

fn find_ocara_bin() -> PathBuf {
    // 1. Variable d'environnement OCARA
    if let Ok(v) = std::env::var("OCARA") {
        let p = PathBuf::from(v);
        if p.exists() { return p; }
    }
    // 2. Relatif à l'emplacement de ocaraunit
    let candidates = [
        "./target/release/ocara",
        "../target/release/ocara",
        "../../target/release/ocara",
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() { return p; }
    }
    // 3. PATH
    PathBuf::from("ocara")
}

/// Génère un wrapper source qui appelle uniquement la fonction/méthode `test_name`
/// depuis le fichier source `src_path`, et le compile + exécute.
///
/// Stratégie : on passe l'argument `--entry <test_name>` au compilateur ocara
/// si celui-ci le supporte. Sinon on compile le fichier entier avec `--test <name>`.
/// Pour la simplicité, on utilise le flag `--run-test <name>` défini comme
/// convention dans le compilateur Ocara.
///
/// Puisque le compilateur Ocara n'a pas encore ce flag, ocaraunit utilise
/// une approche simple : compiler le fichier avec `-o tmp` puis appeler
/// le binaire avec l'argument `<test_name>`. Si le binaire ne comprend pas
/// d'argument, on ne peut pas isoler les tests — on les exécute tous à la fois.
///
/// → Pour cette version, on compile le fichier et on exécute le binaire
///   sans argument. Le runtime UnitTest écrit PASS/FAIL sur stdout.
///   On ne peut pas isoler les tests à ce stade sans support du compilateur.
///   On exécute donc tout le fichier *Test.oc et on parse toute la sortie.
fn run_test_file(
    ocara: &Path,
    src: &Path,
    tmp_dir: &Path,
    project_root: &Path,
) -> (Vec<TestResult>, Option<String>) {
    let stem = src.file_stem().unwrap_or_default().to_string_lossy();
    let bin = tmp_dir.join(stem.as_ref());

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
    let compile = Command::new(ocara)
        .args([tmp_src.as_os_str(), std::ffi::OsStr::new("-o"), bin.as_os_str()])
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let _ = fs::remove_file(&tmp_src); // nettoyage immédiat

    let compile_out = match compile {
        Err(e) => return (vec![], Some(format!("impossible de lancer ocara : {}", e))),
        Ok(o)  => o,
    };

    if !compile_out.status.success() {
        let stderr = String::from_utf8_lossy(&compile_out.stderr).to_string();
        return (vec![], Some(stderr.trim().to_string()));
    }

    // Exécution
    let run_out = Command::new(&bin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let _ = fs::remove_file(&bin); // nettoyage

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
        Some(stderr.trim().to_string())
    } else {
        None
    };

    (vec![TestResult { asserts: all_asserts, error }], None)
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
    eprintln!("Usage: ocaraunit [--coverage [<dossier>]]");
    eprintln!();
    eprintln!("  --coverage [<dossier>]  Analyse la couverture des sources de <dossier> (défaut : .)");
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
            "--help" | "-h" => { usage(); std::process::exit(0); }
            _ => {}
        }
    }

    // Les tests sont TOUJOURS dans ./tests/ (cwd de lancement)
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let tests_dir = cwd.join("tests");

    if !tests_dir.exists() {
        println!("{}Aucun dossier tests/ trouvé à la racine ({}){}",
            c.yellow, cwd.display(), c.reset);
        println!("{}Créez un dossier tests/ et placez-y vos fichiers *Test.oc{}", c.dim, c.reset);
        std::process::exit(0);
    }

    let scan_root = fs::canonicalize(&tests_dir).unwrap_or(tests_dir.clone());

    // Dossier source pour la couverture (argument ou cwd)
    let coverage_source = fs::canonicalize(
        PathBuf::from(coverage_source_arg.unwrap_or_else(|| ".".to_string()))
    ).unwrap_or_else(|_| cwd.clone());

    // Chercher la racine de config en remontant depuis cwd
    let config_root = find_config_root(&cwd).unwrap_or_else(|| cwd.clone());
    let cfg = load_config(&config_root);

    let ocara = find_ocara_bin();

    // Répertoire temporaire pour les binaires compilés
    let tmp_dir = std::env::temp_dir().join("ocaraunit_bins");
    let _ = fs::create_dir_all(&tmp_dir);

    // ── Découverte des fichiers de test ──────────────────────────────────────
    let test_files = collect_test_files(&scan_root, &cfg.exclude, &config_root);

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

        let (results, compile_err) = run_test_file(&ocara, tf, &tmp_dir, &cwd);

        if let Some(err) = compile_err {
            println!("  {}ERREUR compilation :{} {}", c.red, c.reset, err.lines().next().unwrap_or(""));
            total_errors += 1;
            has_failure = true;
            continue;
        }

        for res in &results {
            if let Some(ref err) = res.error {
                println!("  {}ERREUR exécution :{} {}", c.red, c.reset, err.lines().next().unwrap_or(""));
                total_errors += 1;
                has_failure = true;
                continue;
            }

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
            println!("  {}{} PASS  {} FAIL{}", col, p, f, c.reset);
        }
    }

    println!();
    print_separator(&c);
    let global_col = if has_failure { c.red } else { c.green };
    println!("{}Résultat global : {} PASS  {} FAIL  {} ERREUR(S){}",
        global_col, total_pass, total_fail, total_errors, c.reset);
    print_separator(&c);
    } // fin else test_files non vide

    // ── Analyse de couverture ────────────────────────────────────────────────
    if coverage_mode {
        println!();
        print_separator(&c);
        println!("{} Couverture de tests{}", c.bold, c.reset);
        print_separator(&c);
        println!();

        let all_oc = collect_oc_files(&coverage_source, &cfg.exclude, &config_root);
        // Exclure les fichiers *Test.oc eux-mêmes et le dossier tests/
        let test_file_set: HashSet<PathBuf> = test_files.iter().cloned().collect();
        let source_files: Vec<PathBuf> = all_oc
            .into_iter()
            .filter(|p| !test_file_set.contains(p))
            .collect();

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

    // Nettoyage du dossier temporaire
    let _ = fs::remove_dir_all(&tmp_dir);

    std::process::exit(if has_failure { 1 } else { 0 });
}
