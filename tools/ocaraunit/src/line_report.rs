// ─────────────────────────────────────────────────────────────────────────────
// Couverture par ligne : périmètre, points des fichiers jamais compilés par
// un test, affichage (voir `lines.rs`)
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    config::Config,
    lines::{percent, FileLines, Hits, Unanalyzed},
    process::{run, strip_ansi},
    report::{coverage_sources, print_separator, rel_path, Ansi},
    runner::RunContext,
};

/// Fichiers runtime (`*.runtime.oc`) en dernier : leurs points viennent du
/// fichier qui les déclare (`main.oc`), ils ne se compilent pas seuls.
fn analysis_order(sources: &mut [PathBuf]) {
    sources.sort_by_key(|p| p.to_string_lossy().ends_with(".runtime.oc"));
}

/// Points d'un fichier jamais compilé par un test (`ocara --coverage-points`).
fn query_points(ctx: &RunContext, source: &Path, hits: &mut Hits) -> Result<(), String> {
    let mut cmd = Command::new(&ctx.ocara);
    cmd.arg(source).arg("--coverage-points").arg("--src").arg(&ctx.project_root).current_dir(&ctx.project_root);
    let out = run(&mut cmd, ctx.timeout).map_err(|e| format!("impossible de lancer {} : {}", ctx.ocara.display(), e))?;
    if out.timed_out {
        return Err(format!("délai de {} s dépassé", ctx.timeout.as_secs()));
    }
    if !out.status.success() {
        let text = strip_ansi(&format!("{}{}", out.stderr, out.stdout));
        return Err(text.lines().take(5).collect::<Vec<_>>().join("\n"));
    }
    hits.add_points(&out.stdout, &ctx.project_root);
    Ok(())
}

/// Couverture par ligne des fichiers de `scope` (hors tests et exclusions).
pub fn compute(ctx: &RunContext, scope: &Path, scan_root: &Path, test_files: &[PathBuf], cfg: &Config,
               cfg_root: &Path, hits: &mut Hits) -> (Vec<FileLines>, Vec<Unanalyzed>) {
    let mut sources: Vec<PathBuf> = coverage_sources(scope, scan_root, test_files, cfg, cfg_root).into_iter()
        .map(|p| fs::canonicalize(&p).unwrap_or(p))
        .collect();
    analysis_order(&mut sources);
    let mut unanalyzed = Vec::new();
    for source in &sources {
        if hits.files.contains_key(source) { continue; }
        if let Err(error) = query_points(ctx, source, hits) {
            unanalyzed.push(Unanalyzed { path: source.clone(), error });
        }
    }
    unanalyzed.retain(|u| !hits.files.contains_key(&u.path));
    let mut files: Vec<FileLines> = sources.iter()
        .filter_map(|s| hits.files.get(s).map(|lines| FileLines::of(s, lines)))
        .filter(|f| f.total > 0)
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    (files, unanalyzed)
}

fn color<'a>(pct: f64, c: &'a Ansi) -> &'a str {
    if pct >= 100.0 { c.green } else if pct >= 75.0 { c.yellow } else { c.red }
}

pub fn print(c: &Ansi, files: &[FileLines], unanalyzed: &[Unanalyzed]) {
    println!();
    print_separator(c);
    println!("{} Couverture par ligne{}", c.bold, c.reset);
    print_separator(c);
    println!();
    let width = files.iter().map(|f| rel_path(&f.path).len()).max().unwrap_or(0).max(20);
    for f in files {
        let pct = f.percent();
        let filled = ((pct / 100.0) * 20.0).round() as usize;
        let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(20usize.saturating_sub(filled)));
        let path = rel_path(&f.path);
        println!("  {:<width$}  {}{:>6.1}%  {}  {}/{} lignes{}", path, color(pct, c), pct, bar, f.covered, f.total, c.reset, width = width);
        for (start, end) in &f.uncovered {
            let lines = if start == end { format!("ligne {}", start) } else { format!("lignes {}–{}", start, end) };
            println!("    {}{}:{}:1: non couverte ({}){}", c.red, path, start, lines, c.reset);
        }
    }
    for u in unanalyzed {
        println!("  {}{} : non analysé — {}{}", c.yellow, rel_path(&u.path), u.error.lines().next().unwrap_or(""), c.reset);
    }
    if files.is_empty() && unanalyzed.is_empty() {
        println!("  {}Aucun fichier source trouvé.{}", c.dim, c.reset);
    }
    let covered: usize = files.iter().map(|f| f.covered).sum();
    let total: usize = files.iter().map(|f| f.total).sum();
    let pct = percent(covered, total);
    println!();
    print_separator(c);
    println!("{} Couverture globale : {}{:.1}%{} ({}/{} lignes){}", c.bold, color(pct, c), pct, c.bold, covered, total, c.reset);
    print_separator(c);
}
