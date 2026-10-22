// ─────────────────────────────────────────────────────────────────────────────
// `RuntimeTest.oc` : test du `main` complet généré par les blocs runtime du
// point d'entrée (`main.oc` à la racine du projet) ; `<entrée>.RuntimeTest.oc`
// vise un autre point d'entrée de la racine (`probe_main.RuntimeTest.oc` →
// `probe_main.oc`).
//
// Le source compilé est le fichier de test, suivi des déclarations de
// `main.oc` (imports, `runtime … is …`) et d'un dernier
// `runtime __ocaraunit__<stem>_runner is exit` : le flot complet s'exécute
// (init → main → error|success → exit), puis le runner transmet `ERROR` à
// `UnitTest` et appelle les tests. `ERROR` est remis à 0 : le code de sortie
// du binaire ne reflète plus que les tests. Une méthode `public static
// prepareRuntime()` de la classe de test est appelée avant `init` (bloc
// `init` écrit dans le source, placé avant ceux des fichiers runtime) :
// elle prépare l'environnement d'un scénario (port occupé, base…). Un
// `RuntimeTest.oc` par scénario, dans des dossiers distincts.
//
// Une compilation et une exécution par variante `when` des déclarations
// `runtime` du point d'entrée (`--when System::OS=android` sur l'hôte).
// ─────────────────────────────────────────────────────────────────────────────

use std::path::{Path, PathBuf};

use crate::discovery::{declarations, test_calls, TestEntry};

/// Nom du fichier de test d'un point d'entrée.
pub const RUNTIME_TEST_FILE: &str = "RuntimeTest.oc";
/// Point d'entrée exécuté par `RuntimeTest.oc`.
pub const ENTRY_FILE: &str = "main.oc";

/// Point d'entrée testé par `test`, si c'est un `RuntimeTest.oc` ou un
/// `<entrée>.RuntimeTest.oc`.
pub fn entry_of(test: &Path, project_root: &Path) -> Option<PathBuf> {
    let name = test.file_name()?.to_str()?;
    if name == RUNTIME_TEST_FILE {
        return Some(project_root.join(ENTRY_FILE));
    }
    let entry = name.strip_suffix(RUNTIME_TEST_FILE)?.strip_suffix('.')?;
    (!entry.is_empty()).then(|| project_root.join(format!("{}.oc", entry)))
}

/// Constantes de build imposées au compilateur (`--when`) ; vide : hôte.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Variant(pub Vec<(String, String)>);

impl Variant {
    /// `System::OS = 'android'` (vide pour l'hôte).
    pub fn label(&self) -> String {
        self.0.iter().map(|(n, v)| format!("System::{} = '{}'", n, v)).collect::<Vec<_>>().join(", ")
    }

    /// Fragment de nom de fichier de cache (`OS-android`).
    pub fn slug(&self) -> String {
        self.0.iter().map(|(n, v)| format!("{}-{}", n, v)).collect::<Vec<_>>().join(".")
    }

    pub fn args(&self) -> Vec<String> {
        self.0.iter().flat_map(|(n, v)| ["--when".to_string(), format!("System::{}={}", n, v)]).collect()
    }
}

fn code(line: &str) -> &str {
    line.split("//").next().unwrap_or("").trim()
}

/// Constantes imposées par une clause `when` : ses comparaisons `is` du
/// premier membre d'un `or`. `None` : la configuration de l'hôte convient
/// (`default`, `is not`, comparaisons d'ordre).
fn clause_values(clause: &str) -> Option<Vec<(String, String)>> {
    let first = clause.split(" or ").next()?;
    let mut values = Vec::new();
    for atom in first.split(" and ") {
        let atom = atom.trim().trim_matches(|c| c == '(' || c == ')');
        let Some(rest) = atom.strip_prefix("System::") else { return None };
        let (name, cmp) = rest.split_once(char::is_whitespace)?;
        let value = cmp.trim().strip_prefix("is ")?.trim();
        if !(value.starts_with('\'') || value.starts_with('"')) {
            return None;
        }
        values.push((name.to_string(), value.trim_matches(|c| c == '\'' || c == '"').to_string()));
    }
    Some(values)
}

/// Variantes des déclarations `runtime` d'un point d'entrée : l'hôte
/// d'abord, puis une par clause `when` qui impose d'autres constantes.
pub fn variants(entry_source: &str) -> Vec<Variant> {
    let lines: Vec<&str> = entry_source.lines().map(code).collect();
    let mut out = vec![Variant::default()];
    for (idx, line) in lines.iter().enumerate() {
        if !line.starts_with("runtime ") {
            continue;
        }
        let mut values = Vec::new();
        for clause in lines[..idx].iter().rev().filter(|l| !l.is_empty()).take_while(|l| l.starts_with("when ")) {
            values.extend(clause_values(&clause["when ".len()..]).unwrap_or_default());
        }
        let variant = Variant(values);
        if !out.contains(&variant) {
            out.push(variant);
        }
    }
    out
}

/// Variantes à exécuter pour un fichier de test : celles du point d'entrée
/// d'un `RuntimeTest.oc`, l'hôte seul sinon.
pub fn test_variants(test: &Path, project_root: &Path) -> Vec<Variant> {
    entry_of(test, project_root)
        .and_then(|entry| std::fs::read_to_string(entry).ok())
        .map(|source| variants(&source))
        .unwrap_or_else(|| vec![Variant::default()])
}

/// Vrai si le point d'entrée déclare au moins un bloc runtime.
pub fn declares_runtime(entry_source: &str) -> bool {
    entry_source.lines().map(code).any(|l| l.starts_with("runtime "))
}

/// Module runtime du runner (`__ocaraunit__<stem>_runner`) ; un `.` du stem
/// (`probe_main.RuntimeTest`) serait lu comme un dossier.
pub fn runner_module(stem: &str) -> String {
    format!("__ocaraunit__{}_runner", stem.replace('.', "_"))
}

/// Méthode d'une classe de test appelée avant le flot runtime.
pub const PREPARE_HOOK: &str = "prepareRuntime";

/// `Classe::prepareRuntime()` si la classe de test la déclare (publique, statique).
fn prepare_call(test_source: &str) -> Option<String> {
    declarations(test_source).into_iter()
        .find(|d| d.name == PREPARE_HOOK && d.is_public && d.is_static)
        .and_then(|d| d.class)
        .map(|class| format!("{}::{}()", class, PREPARE_HOOK))
}

/// Source compilé : le test (lignes conservées), son éventuel `init`
/// préparatoire, puis les déclarations du point d'entrée sans son
/// `namespace`, puis le runner en dernier bloc `exit`.
pub fn compose(test_source: &str, entry_source: &str, runner: &str) -> String {
    let entry: Vec<&str> = entry_source.lines().filter(|l| !code(l).starts_with("namespace ")).collect();
    let prepare = prepare_call(test_source).map(|call| format!("init {{\n    {}\n}}\n", call)).unwrap_or_default();
    format!("{}\n{}{}\nimport ocara.UnitTest\nruntime {} is exit\n", test_source, prepare, entry.join("\n"), runner)
}

/// Bloc `exit` du runner : résultat du flot, tests, puis `ERROR = 0`.
pub fn runner_source(tests: &[TestEntry]) -> String {
    format!("UnitTest::__recordRuntimeResult(ERROR)\n{}ERROR = 0\n", test_calls(tests))
}
