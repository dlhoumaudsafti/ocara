// ─────────────────────────────────────────────────────────────────────────────
// Fichiers référencés par un fichier `.oc` : imports utilisateur et imports
// runtime, résolus comme le compilateur (src/core/analysis.d/imports.rs,
// src/core/runtime_expand.rs) :
//   - `import a.b.C` et `import X from "a/b"` : depuis le namespace du
//     fichier (`namespace n` → `<racine>/n/…`), sinon depuis la racine (le
//     dossier du fichier d'entrée) ;
//   - `import X from "./…"` / `"../…"` : relatif au fichier qui importe ;
//   - `runtime a.b (is bloc)?` : `<racine>/a/b.runtime.oc`, `.run.oc`,
//     `.rt.oc`, puis `.oc` ;
//   - `wiring a.b.C` (interface, classe de test) : import implicite de
//     `a/b/C`, résolu comme `import a.b.C`.
// ─────────────────────────────────────────────────────────────────────────────

use std::path::{Path, PathBuf};

use crate::text::{line_states, masked_code};

/// Modules livrés avec le runtime (importables aussi sans `ocara.`).
const OCARA_BUILTINS: &[&str] = &[
    "IO", "Math", "String", "Array", "Map", "JSON", "Tauri", "SDL",
    "Convert", "System", "Regex", "HTTPRequest", "HTTPResponse", "HTTPServer", "HTTPServerRequest", "HTTPServerSession", "SQLite", "MySQL", "MariaDB", "DotEnv", "YAML", "Thread", "Mutex",
    "DateTime", "Date", "Time", "UnitTest", "HTMLComponent", "HTML",
    "File", "Directory", "Exception", "FileException", "DirectoryException", "IOException", "SystemException",
    "ArrayException", "MapException", "MathException", "ConvertException", "RegexException",
    "DateTimeException", "DateException", "TimeException",
    "ThreadException", "MutexException",
    "UnitTestException", "HTTPServerException", "SQLiteException", "MySQLException", "MariaDBException", "DotEnvException", "YAMLException",
    "TauriException", "SDLException",
];

const RUNTIME_EXTENSIONS: [&str; 4] = ["runtime.oc", "run.oc", "rt.oc", "oc"];

/// Contexte de résolution d'un fichier.
struct Resolver<'a> {
    file_dir:  &'a Path,
    root:      &'a Path,
    namespace: Option<String>,
}

impl Resolver<'_> {
    /// Chemin d'un import (`a/b` ou `./a`, sans extension obligatoire).
    fn import(&self, path: &str) -> PathBuf {
        let clean = path.trim_end_matches(".oc");
        if clean.starts_with("./") || clean.starts_with("../") {
            return self.file_dir.join(clean).with_extension("oc");
        }
        if let Some(ns) = &self.namespace {
            let in_namespace = self.root.join(ns.replace('.', "/")).join(clean).with_extension("oc");
            if in_namespace.exists() { return in_namespace; }
        }
        self.root.join(clean).with_extension("oc")
    }

    /// Fichier d'un `runtime a.b` (premier suffixe existant).
    fn runtime(&self, segments: &str) -> PathBuf {
        let base = segments.replace('.', "/");
        RUNTIME_EXTENSIONS.iter()
            .map(|ext| self.root.join(format!("{}.{}", base, ext)))
            .find(|p| p.exists())
            .unwrap_or_else(|| self.root.join(format!("{}.oc", base)))
    }

    /// Fichier désigné par une ligne `import …`/`runtime …` (code seul).
    fn line_target(&self, code: &str, raw: &str) -> Option<PathBuf> {
        if let Some(rest) = code.strip_prefix("runtime ") {
            let module = rest.split_whitespace().next()?;
            return Some(self.runtime(module));
        }
        // `wiring a.b.C` (interface, classe de test) : import implicite.
        if let Some(rest) = code.strip_prefix("wiring ") {
            let module = rest.split_whitespace().next()?;
            return Some(self.import(&module.replace('.', "/")));
        }
        let rest = code.strip_prefix("import ")?;
        let main = rest.split(" as ").next().unwrap_or(rest).trim();
        if let Some(path) = quoted_path(code, raw) {
            return Some(self.import(&path));
        }
        let first = main.split('.').next().unwrap_or("");
        if first == "ocara" || OCARA_BUILTINS.contains(&first) { return None; }
        Some(self.import(&main.replace('.', "/")))
    }
}

/// Chemin entre guillemets d'un `import X from "…"` : `code` a le texte des
/// chaînes masqué, `raw` est la ligne d'origine.
fn quoted_path(code: &str, raw: &str) -> Option<String> {
    let from = code.find(" from ")?;
    let raw_chars: Vec<char> = raw.chars().collect();
    let start = code[..from].chars().count() + " from ".len();
    let rest: String = raw_chars.get(start..)?.iter().collect();
    let inner = rest.trim_start().strip_prefix('"')?;
    Some(inner.split('"').next()?.to_string())
}

/// Fichiers `.oc` référencés par `content` (fichier de dossier `file_dir`),
/// imports résolus depuis `root`.
pub fn referenced_files(content: &str, file_dir: &Path, root: &Path) -> Vec<PathBuf> {
    let lines: Vec<&str> = content.lines().collect();
    let states = line_states(&lines);
    let mut resolver = Resolver { file_dir, root, namespace: None };
    let mut in_bt = false;
    let mut out = Vec::new();
    for (line, state) in lines.iter().zip(&states) {
        let code = masked_code(line, &mut in_bt);
        if state.starts_in_bt { continue; }
        let code = code.trim();
        if let Some(ns) = code.strip_prefix("namespace ").map(str::trim) {
            resolver.namespace = (ns != ".").then(|| ns.to_string());
        } else if let Some(path) = resolver.line_target(code, line.trim()) {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::referenced_files;
    use std::path::{Path, PathBuf};

    #[test]
    fn imports_resolve_from_the_root_or_the_file() {
        let src = "import ocara.IO\nimport IO\nimport lib.Car // voiture\nimport Truck from \"./Truck\" as T\nimport * from \"shapes/All\"\nruntime config is init\n    wiring infra.Repo\nvar s:string = `\nimport fake.X\n`\n";
        let got = referenced_files(src, Path::new("/p/sub"), Path::new("/p"));
        let want: Vec<PathBuf> = ["/p/lib/Car.oc", "/p/sub/Truck.oc", "/p/shapes/All.oc", "/p/config.oc", "/p/infra/Repo.oc"].iter().map(PathBuf::from).collect();
        assert_eq!(got, want);
    }
}
