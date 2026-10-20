// ─────────────────────────────────────────────────────────────────────────────
// Affichage des résultats et de la couverture
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    collections::HashSet,
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
};

use crate::{
    config::{collect_oc_files, Config},
    coverage::{compute_coverage, extract_all_function_names, FileCoverage},
    lines::Hits,
    runner::{run_test_file, RunContext, TestResult},
};

pub struct Ansi {
    pub green:  &'static str,
    pub yellow: &'static str,
    pub red:    &'static str,
    pub bold:   &'static str,
    pub reset:  &'static str,
    pub dim:    &'static str,
}

/// Couleurs seulement vers un terminal, et sans `NO_COLOR`.
pub fn ansi() -> Ansi {
    let color = std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err();
    if color {
        Ansi { green: "\x1b[0;32m", yellow: "\x1b[0;33m", red: "\x1b[0;31m",
               bold: "\x1b[1m", reset: "\x1b[0m", dim: "\x1b[2m" }
    } else {
        Ansi { green: "", yellow: "", red: "", bold: "", reset: "", dim: "" }
    }
}

fn coverage_color<'a>(pct: f64, c: &'a Ansi) -> &'a str {
    if pct >= 100.0 { c.green } else if pct >= 75.0 { c.yellow } else { c.red }
}

pub fn rel_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.strip_prefix(&cwd).unwrap_or(path).to_string_lossy().to_string()
}

pub fn print_separator(c: &Ansi) {
    println!("{}══════════════════════════════════════════════{}", c.dim, c.reset);
}

fn print_block(c: &Ansi, title: &str, text: &str) {
    println!("  {}{}{}", c.red, title, c.reset);
    for line in text.lines() {
        println!("  {}", line);
    }
}

#[derive(Default)]
pub struct Totals {
    pub pass:   usize,
    pub fail:   usize,
    pub errors: usize,
    compile_ms: u128,
    run_ms:     u128,
}

impl Totals {
    pub fn has_failure(&self) -> bool {
        self.fail > 0 || self.errors > 0
    }
}

/// Affiche le résultat d'un fichier de test et l'ajoute aux totaux.
fn print_result(c: &Ansi, res: &TestResult, totals: &mut Totals) {
    totals.compile_ms += res.compile_time_ms;
    totals.run_ms += res.run_time_ms;
    if res.asserts.is_empty() && res.error.is_none() {
        println!("  {}(aucune assertion){}", c.dim, c.reset);
    }
    for a in &res.asserts {
        let (col, label) = if a.passed { (c.green, "PASS") } else { (c.red, "FAIL") };
        println!("  {}{}{} {}", col, label, c.reset, a.message);
    }
    let (p, f) = (res.pass_count(), res.fail_count());
    totals.pass += p;
    totals.fail += f;
    let col = if f == 0 && res.error.is_none() { c.green } else { c.red };
    let time_info = if res.compile_time_ms > 0 {
        format!("(compile: {}ms, run: {}ms)", res.compile_time_ms, res.run_time_ms)
    } else {
        format!("(cached, run: {}ms)", res.run_time_ms)
    };
    println!("  {}{} PASS  {} FAIL{} {}{}{}", col, p, f, c.reset, c.dim, time_info, c.reset);
    // Après les assertions déjà passées : l'exécution s'est arrêtée là.
    if let Some(ref err) = res.error {
        print_block(c, "ERREUR exécution :", err);
        totals.errors += 1;
    }
}

fn print_totals(c: &Ansi, t: &Totals) {
    println!();
    print_separator(c);
    let col = if t.has_failure() { c.red } else { c.green };
    println!("{}Résultat global : {} PASS  {} FAIL  {} ERREUR(S){}", col, t.pass, t.fail, t.errors, c.reset);
    if t.compile_ms > 0 || t.run_ms > 0 {
        println!("{}Temps total : {:.2}s (compile: {:.2}s, run: {:.2}s){}", c.dim,
            (t.compile_ms + t.run_ms) as f64 / 1000.0, t.compile_ms as f64 / 1000.0, t.run_ms as f64 / 1000.0, c.reset);
    }
    print_separator(c);
}

/// Exécute chaque fichier de test, affiche les résultats et les totaux.
pub fn run_all(c: &Ansi, ctx: &RunContext, test_files: &[PathBuf], hits: &mut Hits) -> Totals {
    print_separator(c);
    println!("{} Tests ocaraunit{}", c.bold, c.reset);
    print_separator(c);
    let mut totals = Totals::default();
    for tf in test_files {
        let shown = rel_path(tf);
        println!();
        println!("{}{}{}:", c.bold, shown, c.reset);
        match run_test_file(ctx, tf, &shown) {
            Ok(res) => {
                if let Some(report) = &res.coverage {
                    hits.add_report(report, &ctx.project_root);
                }
                print_result(c, &res, &mut totals);
            }
            Err(err) => {
                print_block(c, "ERREUR compilation :", &err);
                totals.errors += 1;
            }
        }
    }
    print_totals(c, &totals);
    totals
}

/// Fichiers source couverts : `scope` (fichier unique ou dossier), hors
/// fichiers de test et hors dossier de tests (fichiers de support compris).
pub fn coverage_sources(scope: &Path, scan_root: &Path, test_files: &[PathBuf], cfg: &Config, cfg_root: &Path) -> Vec<PathBuf> {
    if scope.is_file() {
        return vec![scope.to_path_buf()];
    }
    let tests: HashSet<&PathBuf> = test_files.iter().collect();
    collect_oc_files(scope, &cfg.exclude, cfg_root).into_iter()
        .filter(|p| !tests.contains(p))
        .filter(|p| !fs::canonicalize(p).is_ok_and(|abs| abs.starts_with(scan_root)))
        .filter(|p| !p.to_string_lossy().contains("__ocaraunit__"))
        .collect()
}

fn print_file_coverage(c: &Ansi, cov: &FileCoverage, width: usize) {
    let pct = cov.percent();
    let filled = ((pct / 100.0) * 20.0).round() as usize;
    let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(20usize.saturating_sub(filled)));
    let path = rel_path(&cov.path);
    println!("  {:<width$}  {}{:>6.1}%  {}  {}/{} fonctions{}",
        path, coverage_color(pct, c), pct, bar, cov.covered_funcs, cov.total_funcs, c.reset, width = width);
    for (fname, lineno) in &cov.uncovered {
        println!("    {}{}:{}:1: non couverte — manque {}Test{}", c.red, path, lineno, fname, c.reset);
    }
}

/// Affiche la couverture de `scope` par les tests nommés `test_names`.
pub fn print_coverage(c: &Ansi, scope: &Path, scan_root: &Path, test_files: &[PathBuf], cfg: &Config,
                      cfg_root: &Path, test_names: &HashSet<String>) {
    println!();
    print_separator(c);
    println!("{} Couverture de tests{}", c.bold, c.reset);
    print_separator(c);
    println!();
    let coverages: Vec<FileCoverage> = coverage_sources(scope, scan_root, test_files, cfg, cfg_root).iter()
        .filter_map(|sf| fs::read_to_string(sf).ok().map(|src| (sf, src)))
        .map(|(sf, src)| compute_coverage(sf, &extract_all_function_names(&src), test_names))
        .collect();
    let width = coverages.iter().map(|cv| rel_path(&cv.path).len()).max().unwrap_or(0).max(20);
    for cov in &coverages {
        print_file_coverage(c, cov, width);
    }
    if coverages.is_empty() {
        println!("  {}Aucun fichier source trouvé.{}", c.dim, c.reset);
    }
    let total: usize = coverages.iter().map(|cv| cv.total_funcs).sum();
    let covered: usize = coverages.iter().map(|cv| cv.covered_funcs).sum();
    let pct = if total == 0 { 100.0 } else { covered as f64 / total as f64 * 100.0 };
    println!();
    print_separator(c);
    println!("{} Couverture globale : {}{:.1}%{} ({}/{} fonctions){}",
        c.bold, coverage_color(pct, c), pct, c.bold, covered, total, c.reset);
    print_separator(c);
}
