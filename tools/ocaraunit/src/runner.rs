// ─────────────────────────────────────────────────────────────────────────────
// Compilation et exécution d'un fichier de test
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use crate::{
    cache::{purge, test_key, EntryName},
    discovery::{declares_main, extract_tests, generate_main},
    imports::rebase_imports,
    process::{describe_status, run, strip_ansi, Output},
    runtime_test::{self, Variant},
};

/// Variable d'environnement des binaires de test : les builtins bloquants
/// y rendent la main (voir runtime/src/blocking.rs).
const TEST_MODE_ENV: &str = "OCARA_UNITTEST";

/// Paramètres communs à tous les fichiers de test d'une exécution.
pub struct RunContext {
    pub ocara:        PathBuf,
    pub project_root: PathBuf,
    pub cache_dir:    PathBuf,
    pub fingerprint:  u64,
    pub timeout:      Duration,
    /// Conserver les codes ANSI des sorties du compilateur et du binaire
    pub color:        bool,
    /// Binaires compilés avec `ocara --coverage` (voir `lines.rs`)
    pub line_coverage: bool,
}

#[derive(Debug)]
pub struct AssertResult {
    pub passed:  bool,
    pub message: String,
}

#[derive(Debug)]
pub struct TestResult {
    pub asserts: Vec<AssertResult>,
    /// Fin anormale de l'exécution (exception, signal, délai dépassé)
    pub error:   Option<String>,
    /// Temps de compilation en millisecondes (0 : binaire en cache)
    pub compile_time_ms: u128,
    /// Temps d'exécution en millisecondes
    pub run_time_ms: u128,
    /// Points de couverture écrits par le binaire (`--line-coverage`)
    pub coverage: Option<String>,
    /// Builtins bloquants neutralisés pendant l'exécution (`BLOCKED …`)
    pub blocked: Vec<String>,
}

impl TestResult {
    pub fn pass_count(&self) -> usize { self.asserts.iter().filter(|a| a.passed).count() }
    pub fn fail_count(&self) -> usize { self.asserts.iter().filter(|a| !a.passed).count() }
}

impl RunContext {
    fn clean(&self, text: &str) -> String {
        if self.color { text.to_string() } else { strip_ansi(text) }
    }
}

/// Source prêt à compiler : imports réécrits et `main` généré — ou, pour un
/// `RuntimeTest.oc`, déclarations du point d'entrée et runner du bloc `exit`.
struct Prepared {
    source: String,
    /// Contenu du module runtime du runner (`RuntimeTest.oc` seulement)
    runner: Option<String>,
}

/// Erreur si le fichier ne contient aucun test ou déclare déjà son propre `main`.
fn prepare_source(src: &Path, root: &Path, runner_module: &str) -> Result<Prepared, String> {
    let source = fs::read_to_string(src).map_err(|e| format!("impossible de lire le fichier : {}", e))?;
    if declares_main(&source) {
        return Err("le fichier déclare déjà `function main()` : ocaraunit génère le sien \
                    (supprimer ce main, ou renommer le fichier s'il ne s'agit pas d'un test)".to_string());
    }
    let tests = extract_tests(&source);
    if tests.is_empty() {
        return Err("aucun test trouvé : ni fonction, ni méthode publique de classe dont le nom \
                    commence ou se termine par `test`".to_string());
    }
    let test_dir = src.parent().unwrap_or(root);
    let rebased = rebase_imports(&source, test_dir, root);
    let Some(entry) = runtime_test::entry_of(src, root) else {
        return Ok(Prepared { source: format!("{}\n{}", rebased, generate_main(&tests)), runner: None });
    };
    let entry_source = fs::read_to_string(&entry)
        .map_err(|_| format!("{} : point d'entrée {} introuvable", runtime_test::RUNTIME_TEST_FILE, entry.display()))?;
    if !runtime_test::declares_runtime(&entry_source) {
        return Err(format!("{} ne déclare aucun bloc runtime : {} exécute le `main` généré par les blocs runtime",
                           entry.display(), runtime_test::RUNTIME_TEST_FILE));
    }
    Ok(Prepared {
        source: runtime_test::compose(&rebased, &entry_source, runner_module),
        runner: Some(runtime_test::runner_source(&tests)),
    })
}

/// Compile `src` vers `bin`. Le fichier généré `__ocaraunit__<stem>.oc` (et
/// le runner d'un `RuntimeTest.oc`) est placé à la racine (le compilateur y
/// résout les imports) puis supprimé ; les diagnostics qui le citent sont
/// ramenés au fichier de test d'origine.
fn compile(ctx: &RunContext, src: &Path, bin: &Path, shown: &str, variant: &Variant) -> Result<(), String> {
    let stem = src.file_stem().unwrap_or_default().to_string_lossy();
    let runner_module = runtime_test::runner_module(&stem);
    let prepared = prepare_source(src, &ctx.project_root, &runner_module)?;
    let tmp_name = format!("__ocaraunit__{}.oc", stem);
    let tmp_src = ctx.project_root.join(&tmp_name);
    let tmp_runner = ctx.project_root.join(format!("{}.runtime.oc", runner_module));
    fs::write(&tmp_src, prepared.source).map_err(|e| format!("écriture de {} : {}", tmp_src.display(), e))?;
    if let Some(runner) = &prepared.runner {
        fs::write(&tmp_runner, runner).map_err(|e| format!("écriture de {} : {}", tmp_runner.display(), e))?;
    }
    let mut cmd = Command::new(&ctx.ocara);
    cmd.arg(&tmp_src).arg("-o").arg(bin).arg("--src").arg(&ctx.project_root).args(variant.args()).current_dir(&ctx.project_root);
    if ctx.line_coverage {
        cmd.arg("--coverage");
    }
    let out = run(&mut cmd, ctx.timeout);
    let _ = fs::remove_file(&tmp_src);
    let _ = fs::remove_file(&tmp_runner);
    let out = out.map_err(|e| format!("impossible de lancer {} : {}", ctx.ocara.display(), e))?;
    // L'objet intermédiaire n'est jamais gardé dans le cache.
    let _ = fs::remove_file(format!("{}.o", bin.display()));
    if out.timed_out {
        return Err(format!("compilation interrompue : délai de {} s dépassé", ctx.timeout.as_secs()));
    }
    if out.status.success() {
        return Ok(());
    }
    let text = format!("{}{}", out.stderr, out.stdout)
        .replace(&tmp_src.to_string_lossy().to_string(), shown)
        .replace(&tmp_name, shown);
    let lines: Vec<&str> = text.lines().take(15).collect();
    Err(ctx.clean(&lines.join("\n")))
}

/// Builtins bloquants neutralisés (lignes `BLOCKED <builtin>`).
fn parse_blocked(stdout: &str) -> Vec<String> {
    stdout.lines().filter_map(|l| l.strip_prefix("BLOCKED ")).map(str::to_string).collect()
}

/// Lignes `PASS …` / `FAIL …` de la sortie du binaire.
fn parse_asserts(stdout: &str) -> Vec<AssertResult> {
    stdout.lines().filter_map(|line| {
        if let Some(msg) = line.strip_prefix("PASS ") {
            Some(AssertResult { passed: true, message: msg.to_string() })
        } else {
            line.strip_prefix("FAIL ").map(|msg| AssertResult { passed: false, message: msg.to_string() })
        }
    }).collect()
}

/// Message (`Message: …`) d'une `UnitTestException` non rattrapée.
fn uncaught_assertion(stderr: &str) -> Option<String> {
    let plain = strip_ansi(stderr);
    if !plain.contains("UnitTestException") { return None; }
    plain.lines().find_map(|l| l.trim().strip_prefix("Message:").map(|m| m.trim().to_string()))
}

/// Erreur d'exécution d'un binaire terminé anormalement (`None` sinon).
/// Une assertion en échec hors des appels de test (constructeur, `init`)
/// est ajoutée aux FAIL plutôt que comptée en erreur.
fn run_error(ctx: &RunContext, out: &Output, asserts: &mut Vec<AssertResult>) -> Option<String> {
    if out.timed_out {
        return Some(format!("délai de {} s dépassé : binaire tué (boucle infinie ?) — voir --timeout",
                            ctx.timeout.as_secs()));
    }
    if out.status.success() {
        return None;
    }
    if let Some(message) = uncaught_assertion(&out.stderr) {
        asserts.push(AssertResult { passed: false, message });
        return None;
    }
    let lines: Vec<&str> = out.stderr.lines().take(20).collect();
    let mut text = describe_status(&out.status);
    if !lines.is_empty() {
        text = format!("{}\n{}", text, ctx.clean(&lines.join("\n")));
    }
    Some(text)
}

/// Compile (sauf binaire en cache) et exécute un fichier de test.
///
/// Le binaire est mis en cache sous une clé couvrant le test, ses imports
/// transitifs et le compilateur (voir `cache.rs`) ; il est exécuté depuis
/// la racine du projet, comme l'application, avec le délai `--timeout`.
/// `Err` : le fichier n'a pas pu être compilé (ou ne contient aucun test).
pub fn run_test_file(ctx: &RunContext, src: &Path, shown: &str, variant: &Variant) -> Result<TestResult, String> {
    let key = test_key(src, &ctx.project_root, ctx.fingerprint)
        .ok_or_else(|| "impossible de lire le fichier source".to_string())?;
    let entry = EntryName::of(src, &variant.slug());
    let bin = entry.path(&ctx.cache_dir, key);
    let mut compile_time_ms = 0u128;
    if !bin.exists() {
        let start = Instant::now();
        compile(ctx, src, &bin, shown, variant)?;
        compile_time_ms = start.elapsed().as_millis().max(1);
        purge(&ctx.cache_dir, &bin, &entry);
    }
    let report = bin.with_extension("cov");
    let _ = fs::remove_file(&report);
    let mut cmd = Command::new(&bin);
    cmd.current_dir(&ctx.project_root).env(TEST_MODE_ENV, "1");
    if ctx.line_coverage {
        cmd.env("OCARA_COVERAGE_FILE", &report);
    }
    let start = Instant::now();
    let out = run(&mut cmd, ctx.timeout)
        .map_err(|e| format!("impossible d'exécuter le binaire : {}", e))?;
    let run_time_ms = start.elapsed().as_millis();
    let mut asserts = parse_asserts(&out.stdout);
    let blocked = parse_blocked(&out.stdout);
    let error = run_error(ctx, &out, &mut asserts);
    let coverage = ctx.line_coverage.then(|| fs::read_to_string(&report).unwrap_or_default());
    let _ = fs::remove_file(&report);
    Ok(TestResult { asserts, error, compile_time_ms, run_time_ms, coverage, blocked })
}
