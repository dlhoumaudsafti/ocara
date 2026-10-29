// ─────────────────────────────────────────────────────────────────────────────
// Imports d'un fichier de test : réécriture et fermeture transitive
//
// Même résolution que le compilateur (`resolve_import_path`,
// src/core/analysis.d/imports.rs) :
//   - `import X from "./…"` / `"../…"` : relatif au dossier du fichier ;
//   - `import X from "a/b"` et `import a.b.X` : namespace du fichier d'abord,
//     puis racine du projet (`--src`).
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

/// Chemin d'import tel qu'écrit, avant résolution.
#[derive(Debug, PartialEq)]
pub enum ImportPath {
    /// `import X from "chemin"`
    From(String),
    /// `import a.b.X` → `a/b/X`
    Dotted(String),
}

/// Chemin d'import d'une ligne `import …` (`None` pour les builtins `ocara.*`).
pub fn parse_import(line: &str) -> Option<ImportPath> {
    let rest = line.trim_start().strip_prefix("import")?;
    if !rest.starts_with(char::is_whitespace) { return None; }
    if let Some(start) = rest.find("from \"").map(|i| i + 6) {
        let len = rest[start..].find('"')?;
        return Some(ImportPath::From(rest[start..start + len].to_string()));
    }
    let module = rest.split_whitespace().next()?;
    if module == "ocara" || module.starts_with("ocara.") { return None; }
    Some(ImportPath::Dotted(module.replace('.', "/")))
}

/// Namespace déclaré en tête de fichier (`namespace a.b`), hors `namespace .`.
fn namespace(source: &str) -> Option<String> {
    source.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("//"))
        .and_then(|l| l.strip_prefix("namespace "))
        .map(|ns| ns.trim().to_string())
        .filter(|ns| ns != ".")
}

fn with_oc(path: PathBuf) -> PathBuf {
    let mut p = path.into_os_string();
    p.push(".oc");
    PathBuf::from(p)
}

fn resolve(import: &ImportPath, file_dir: &Path, ns: Option<&str>, root: &Path) -> PathBuf {
    let raw = match import { ImportPath::From(p) | ImportPath::Dotted(p) => p };
    let clean = raw.trim_end_matches(".oc");
    if clean.starts_with("./") || clean.starts_with("../") {
        return with_oc(file_dir.join(clean));
    }
    if let Some(ns) = ns {
        let in_ns = with_oc(root.join(ns.replace('.', "/")).join(clean));
        if in_ns.exists() { return in_ns; }
    }
    with_oc(root.join(clean))
}

/// Fichiers `.oc` importés par `entry`, transitivement (`entry` exclu).
/// Les fichiers introuvables (builtins, erreurs) sont ignorés : le
/// compilateur les signalera.
pub fn import_closure(entry: &Path, root: &Path) -> Vec<PathBuf> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut todo = vec![entry.to_path_buf()];
    let mut found = Vec::new();
    while let Some(file) = todo.pop() {
        let Ok(source) = fs::read_to_string(&file) else { continue };
        let dir = file.parent().unwrap_or(root);
        let ns = namespace(&source);
        for import in source.lines().filter_map(parse_import) {
            let path = resolve(&import, dir, ns.as_deref(), root);
            let key = path.canonicalize().unwrap_or(path);
            if key.is_file() && seen.insert(key.clone()) {
                found.push(key.clone());
                todo.push(key);
            }
        }
    }
    found.sort();
    found
}

/// Chemin de `target` relatif au dossier `base` (tous deux absolus et
/// canoniques), avec des `..` si `target` est hors de `base`.
pub fn relative_to(target: &Path, base: &Path) -> PathBuf {
    let t: Vec<Component> = target.components().collect();
    let b: Vec<Component> = base.components().collect();
    let common = t.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut rel = PathBuf::new();
    for _ in common..b.len() { rel.push(".."); }
    for c in &t[common..] { rel.push(c.as_os_str()); }
    rel
}

/// Nouveau chemin d'un import relatif au dossier du test, exprimé
/// relativement à `root` (où le test est recompilé). `None` : laisser tel quel.
fn rebase_path(path: &str, test_dir: &Path, root: &Path) -> Option<String> {
    let clean = path.trim_end_matches(".oc");
    let abs = with_oc(test_dir.join(clean)).canonicalize().ok()?;
    let is_relative = clean.starts_with("./") || clean.starts_with("../");
    // Un chemin non relatif qui existe depuis la racine y est déjà résolu.
    if !is_relative && with_oc(root.join(clean)).exists() {
        return None;
    }
    let rel = relative_to(&abs.with_extension(""), root);
    let rel = rel.to_string_lossy().to_string();
    // Hors de la racine : `../…` est résolu par le compilateur depuis le
    // dossier du fichier généré, c'est-à-dire la racine elle-même.
    Some(rel)
}

/// Réécrit les imports `from "chemin"` relatifs au dossier du fichier de
/// test (`"../Foo"` depuis `tests/`) en chemins relatifs à `project_root` :
/// le test est compilé depuis la racine (`--src`). Un chemin déjà relatif à
/// la racine, ou qui ne désigne aucun fichier, est laissé tel quel. Le
/// nombre de lignes est conservé.
pub fn rebase_imports(source: &str, test_dir: &Path, project_root: &Path) -> String {
    let Ok(root) = project_root.canonicalize() else { return source.to_string() };
    source.lines().map(|line| {
        let Some(ImportPath::From(path)) = parse_import(line) else { return line.to_string() };
        let start = line.find("from \"").map(|i| i + 6).unwrap_or(0);
        match rebase_path(&path, test_dir, &root) {
            Some(new_path) => format!("{}{}{}", &line[..start], new_path, &line[start + path.len()..]),
            None => line.to_string(),
        }
    }).collect::<Vec<_>>().join("\n")
}

/// Réécrit le chemin des interfaces `use UI("chemin")` (ocara.UI) relatif au
/// dossier du test en chemin relatif à `project_root`, où le test est
/// recompilé — même raison que `rebase_imports`. Un chemin qui ne désigne
/// aucun fichier depuis le test est laissé tel quel.
pub fn rebase_ui_paths(source: &str, test_dir: &Path, project_root: &Path) -> String {
    const OPEN: &str = "use UI(\"";
    let Ok(root) = project_root.canonicalize() else { return source.to_string() };
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(at) = rest.find(OPEN) {
        let start = at + OPEN.len();
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find('"') else {
            rest = tail;
            break;
        };
        let path = &tail[..end];
        let rebased = test_dir.join(path).canonicalize().ok()
            .filter(|_| !Path::new(path).is_absolute())
            .map(|abs| relative_to(&abs, &root).to_string_lossy().to_string());
        out.push_str(rebased.as_deref().unwrap_or(path));
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}
