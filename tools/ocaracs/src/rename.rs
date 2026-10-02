// ─────────────────────────────────────────────────────────────────────────────
// `--fix` : renommage des déclarations mal nommées (R07/R08/R09/R12/R13) et
// de TOUS leurs usages dans les fichiers du projet. Un identifiant est
// remplacé dans le code et dans les `${…}` des backticks, jamais dans une
// chaîne ni un commentaire. Un membre (méthode, propriété, constante de
// classe) est aussi renommé après `.`/`::` ; les autres noms seulement hors
// de cette position (`.trim()` d'un builtin n'est jamais touché).
//
// Renommage ignoré (et signalé) : nom cible déjà utilisé dans le projet, mot
// réservé, même nom attendu sous deux styles différents, méthode de test
// ocaraunit dont le nom perdrait son suffixe `Test`, ou nom présent dans un
// fichier non `.oc` du projet (template HTML `${nom}`, script…).
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::decls::{declarations, DeclKind};
use crate::text::line_states;

const KEYWORDS: &[&str] = &[
    "import", "from", "namespace", "as", "var", "scoped", "consumed", "property", "const", "function",
    "method", "class", "struct", "generic", "module", "enum", "interface", "wiring", "extends", "modules",
    "implements", "init", "public", "private", "protected", "static", "if", "elseif", "else", "switch",
    "default", "match", "while", "for", "in", "return", "result", "use", "break", "continue", "try", "on",
    "is", "raise", "emit", "self", "parent", "async", "resolve", "variadic", "runtime", "main", "error",
    "success", "exit", "int", "float", "string", "bool", "mixed", "array", "map", "message", "void",
    "true", "false", "null", "and", "or", "not", "equal", "smaller", "greater", "nameless",
];

#[derive(Clone)]
struct Target {
    new:    String,
    member: bool,
    is_type: bool,
}

#[derive(Default)]
pub struct Plan {
    targets: HashMap<String, Target>,
    /// Lignes de compte rendu (renommages ignorés).
    pub skipped: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Renommages retenus, triés : (ancien, nouveau).
    pub fn renames(&self) -> Vec<(String, String)> {
        let sorted: BTreeMap<&String, &Target> = self.targets.iter().collect();
        sorted.into_iter().map(|(old, t)| (old.clone(), t.new.clone())).collect()
    }

    /// Nouveau chemin d'un fichier qui porte le nom d'une classe renommée
    /// (`car_model.oc` déclarant `class car_model` → `CarModel.oc`).
    pub fn moved_path(&self, path: &Path) -> Option<PathBuf> {
        let stem = path.file_stem()?.to_str()?;
        let target = self.targets.get(stem).filter(|t| t.is_type)?;
        let moved = path.with_file_name(format!("{}.oc", target.new));
        (!moved.exists()).then_some(moved)
    }
}

/// Mots (identifiants) d'un fichier texte non `.oc`, avec ce fichier.
pub fn external_words(path: &Path, content: &str, out: &mut HashMap<String, PathBuf>) {
    for word in content.split(|c: char| !c.is_alphanumeric() && c != '_').filter(|w| !w.is_empty()) {
        out.entry(word.to_string()).or_insert_with(|| path.to_path_buf());
    }
}

/// Plan de renommage pour l'ensemble `files` (chemin, contenu) du projet ;
/// `external` : mots des autres fichiers texte du projet (`external_words`).
pub fn plan(files: &[(PathBuf, String)], external: &HashMap<String, PathBuf>, cfg: &Config) -> Plan {
    let mut wanted: HashMap<String, (HashSet<String>, bool, bool)> = HashMap::new();
    let mut used: HashSet<String> = HashSet::new();
    for (path, content) in files {
        let lines: Vec<&str> = content.lines().collect();
        let states = line_states(&lines);
        for decl in declarations(&path.to_string_lossy(), &lines, &states, cfg) {
            let target = if decl.style.matches(&decl.name) { decl.name.clone() } else { decl.style.convert(&decl.name) };
            let entry = wanted.entry(decl.name).or_default();
            entry.0.insert(target);
            entry.1 |= decl.kind == DeclKind::Member;
            entry.2 |= decl.kind == DeclKind::Type;
        }
        rewrite(content, &HashMap::new(), &mut |name, _| { used.insert(name.to_string()); None });
    }

    let mut plan = Plan::default();
    let mut names: Vec<_> = wanted.into_iter().collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    for (old, (targets, member, is_type)) in names {
        if targets.len() > 1 {
            let mut list: Vec<_> = targets.into_iter().collect();
            list.sort();
            plan.skipped.push(format!("'{}' non renommé : styles différents attendus selon la déclaration ({})", old, list.join(", ")));
            continue;
        }
        let Some(new) = targets.into_iter().next().filter(|n| *n != old) else { continue };
        let reason = if new.is_empty() {
            Some("aucun nom valide dans ce style".to_string())
        } else if KEYWORDS.contains(&new.as_str()) {
            Some(format!("'{}' est un mot réservé", new))
        } else if used.contains(&new) {
            Some(format!("'{}' est déjà utilisé dans le projet", new))
        } else if let Some(path) = external.get(&old) {
            Some(format!("référencé hors des fichiers .oc ({})", path.display()))
        } else if old.ends_with("Test") && !new.ends_with("Test") {
            Some("une méthode de test ocaraunit doit se terminer par 'Test'".to_string())
        } else {
            None
        };
        match reason {
            Some(r) => plan.skipped.push(format!("'{}' non renommé en '{}' : {}", old, new, r)),
            None => { plan.targets.insert(old, Target { new, member, is_type }); }
        }
    }
    plan
}

/// Applique le plan à un fichier ; retourne le contenu et le nombre de
/// remplacements.
pub fn apply(content: &str, plan: &Plan) -> (String, usize) {
    let mut count = 0usize;
    let out = rewrite(content, &plan.targets, &mut |name, after_member_op| {
        let target = plan.targets.get(name)?;
        (target.member || !after_member_op).then(|| { count += 1; target.new.clone() })
    });
    (out, count)
}

enum Mode {
    Code,
    Quote(char),
    Backtick,
    /// `${` … `}` dans un backtick : code, avec profondeur d'accolades.
    Interp(usize),
}

/// Recopie `content` en passant chaque identifiant du code à `visit(nom,
/// précédé de '.'/'::')`, qui peut le remplacer. Les lignes `import` sont
/// traitées à part (`rewrite_import`), `namespace`/`runtime` recopiées.
fn rewrite(content: &str, targets: &HashMap<String, Target>, visit: &mut dyn FnMut(&str, bool) -> Option<String>) -> String {
    let mut out = String::with_capacity(content.len());
    let mut modes = vec![Mode::Code];
    for line in content.split_inclusive('\n') {
        let t = line.trim_start();
        if matches!(modes.last(), Some(Mode::Code)) && modes.len() == 1 {
            if t.starts_with("import ") {
                out.push_str(&rewrite_import(line, targets, visit));
                continue;
            }
            if t.starts_with("namespace ") || t.starts_with("runtime ") {
                out.push_str(line);
                continue;
            }
        }
        rewrite_line(line, &mut modes, &mut out, visit);
    }
    out
}

fn rewrite_line(line: &str, modes: &mut Vec<Mode>, out: &mut String, visit: &mut dyn FnMut(&str, bool) -> Option<String>) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match modes.last_mut() {
            Some(Mode::Quote(q)) => {
                let q = *q;
                out.push(c);
                if c == '\\' { if let Some(n) = next { out.push(n); i += 1; } }
                else if c == q { modes.pop(); }
            }
            Some(Mode::Backtick) => {
                out.push(c);
                if c == '\\' { if let Some(n) = next { out.push(n); i += 1; } }
                else if c == '`' { modes.pop(); }
                else if c == '$' && next == Some('{') { out.push('{'); i += 1; modes.push(Mode::Interp(0)); }
            }
            Some(Mode::Code) | Some(Mode::Interp(_)) => {
                if c == '/' && next == Some('/') {
                    out.extend(&chars[i..]);
                    return;
                }
                if c.is_alphabetic() || c == '_' {
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') { i += 1; }
                    let name: String = chars[start..i].iter().collect();
                    let mut k = start;
                    while k > 0 && chars[k - 1] == ' ' { k -= 1; }
                    let member_op = (k >= 1 && chars[k - 1] == '.') || (k >= 2 && chars[k - 1] == ':' && chars[k - 2] == ':');
                    let digit_prefixed = start > 0 && chars[start - 1].is_ascii_digit();
                    match visit(&name, member_op).filter(|_| !digit_prefixed) {
                        Some(new) => out.push_str(&new),
                        None => out.push_str(&name),
                    }
                    continue;
                }
                out.push(c);
                match (c, modes.last_mut()) {
                    ('"' | '\'', _) => modes.push(Mode::Quote(c)),
                    ('`', _) => modes.push(Mode::Backtick),
                    ('{', Some(Mode::Interp(depth))) => *depth += 1,
                    ('}', Some(Mode::Interp(0))) => { modes.pop(); }
                    ('}', Some(Mode::Interp(depth))) => *depth -= 1,
                    _ => {}
                }
            }
            None => out.push(c),
        }
        i += 1;
    }
}

/// `import a.b.Nom (as Alias)?` : seul le dernier segment (symbole importé)
/// et l'alias sont renommés — les segments précédents sont des dossiers.
/// `import Nom from "chemin/Nom" (as Alias)?` : le symbole, l'alias et le
/// dernier composant du chemin (fichier renommé avec sa classe).
fn rewrite_import(line: &str, targets: &HashMap<String, Target>, visit: &mut dyn FnMut(&str, bool) -> Option<String>) -> String {
    let lead = &line[..line.len() - line.trim_start().len()];
    let ending = &line[line.trim_end().len()..];
    let body = line.trim();
    let mut rename = |name: &str| visit(name, false).unwrap_or_else(|| name.to_string());
    let (main, alias) = match body.split_once(" as ") {
        Some((m, a)) => (m, Some(a.trim())),
        None => (body, None),
    };
    let main = main.strip_prefix("import ").unwrap_or(main).trim();
    let rewritten = if let Some((target, path)) = main.split_once(" from ") {
        let path = path.trim();
        let inner = path.trim_matches('"');
        let (dir, file) = inner.rsplit_once('/').map_or(("", inner), |(d, f)| (d, f));
        let (stem, ext) = file.strip_suffix(".oc").map_or((file, ""), |s| (s, ".oc"));
        let stem = targets.get(stem).filter(|t| t.is_type).map_or(stem.to_string(), |t| t.new.clone());
        let new_path = if dir.is_empty() { format!("{}{}", stem, ext) } else { format!("{}/{}{}", dir, stem, ext) };
        let target = if target == "*" { target.to_string() } else { rename(target) };
        format!("{} from \"{}\"", target, new_path)
    } else if main.starts_with("ocara.") {
        main.to_string()
    } else {
        match main.rsplit_once('.') {
            Some((dirs, last)) => format!("{}.{}", dirs, rename(last)),
            None => rename(main),
        }
    };
    let alias = alias.map(|a| format!(" as {}", rename(a))).unwrap_or_default();
    format!("{}import {}{}{}", lead, rewritten, alias, ending)
}

#[cfg(test)]
mod tests {
    use super::{apply, plan};
    use crate::config::parse_config;
    use std::path::PathBuf;

    fn run(files: &[(&str, &str)]) -> (Vec<String>, Vec<String>) {
        let files: Vec<(PathBuf, String)> = files.iter().map(|(p, c)| (PathBuf::from(p), c.to_string())).collect();
        let plan = plan(&files, &Default::default(), &parse_config(""));
        (files.iter().map(|(_, c)| apply(c, &plan).0).collect(), plan.skipped)
    }

    #[test]
    fn renames_declaration_and_usages_across_files() {
        let (out, _) = run(&[
            ("lib/car_model.oc", "class car_model {\n    public property userName:string\n    public method print_info(): void {\n        IO::writeln(`${self.userName} \"userName\"`)\n    }\n}\n"),
            ("main.oc", "import lib.car_model\nfunction main(): int {\n    var c:car_model = use car_model()\n    c.print_info()\n    var s:string = \"print_info\" // print_info\n    return s.len()\n}\n"),
        ]);
        assert!(out[0].contains("class CarModel {") && out[0].contains("property user_name:string"));
        assert!(out[0].contains("method printInfo()") && out[0].contains("`${self.user_name} \"userName\"`"));
        assert!(out[1].starts_with("import lib.CarModel\n"));
        assert!(out[1].contains("var c:CarModel = use CarModel()") && out[1].contains("c.printInfo()"));
        assert!(out[1].contains("\"print_info\" // print_info") && out[1].contains("s.len()"));
    }

    #[test]
    fn names_used_outside_oc_files_are_skipped() {
        let files = vec![(PathBuf::from("a.oc"), "function main(): int {\n    var errorText:string = \"x\"\n    return 0\n}\n".to_string())];
        let mut external = Default::default();
        super::external_words(std::path::Path::new("home.html"), "<alert message=\"${errorText}\">", &mut external);
        let plan = plan(&files, &external, &parse_config(""));
        assert!(plan.is_empty());
        assert!(plan.skipped[0].contains("home.html"), "{:?}", plan.skipped);
    }

    #[test]
    fn bare_names_are_not_renamed_after_a_dot() {
        let (out, _) = run(&[("a.oc", "function main(): int {\n    var isEmpty:bool = s.isEmpty()\n    return 0\n}\n")]);
        assert!(out[0].contains("var is_empty:bool = s.isEmpty()"));
    }

    #[test]
    fn collisions_and_conflicts_are_skipped() {
        let (out, skipped) = run(&[("a.oc", "function is_adult(): bool { return true }\nfunction isAdult(): bool { return false }\nfunction run_fastTest(): int { return 0 }\n")]);
        assert!(out[0].contains("function is_adult()"));
        assert!(out[0].contains("function runFastTest()"));
        assert!(skipped.iter().any(|s| s.contains("'is_adult'") && s.contains("déjà utilisé")), "{:?}", skipped);
    }
}
