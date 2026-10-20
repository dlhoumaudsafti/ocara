// ─────────────────────────────────────────────────────────────────────────────
// `--fix` : renommage des déclarations mal nommées (R07/R08/R09/R12/R13) et
// de TOUS leurs usages dans les fichiers du projet (réécriture : rewrite.rs).
// Un membre (méthode, propriété, constante de classe) est aussi renommé après
// `.`/`::` quand son receveur est un type du projet (members.rs) ; les autres
// noms seulement hors de cette position (`.trim()` d'un builtin n'est jamais
// touché). Un fichier qui porte le nom d'un symbole renommé
// (`lib/car_model.oc` déclarant `car_model`) est renommé avec lui, et les
// imports qui le désignent aussi.
//
// Renommage ignoré (et signalé) : nom cible déjà utilisé dans le projet ou
// visé par un autre renommage, mot réservé, même nom attendu sous deux styles
// différents, méthode de test ocaraunit dont le nom perdrait son suffixe
// `Test`, nom présent dans un fichier non `.oc` du projet (template HTML
// `${nom}`, script…), membre accédé sur un receveur de type inconnu, nom
// porté aussi par un champ nu de struct, fichier de destination existant.
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::decls::{declarations, struct_fields, DeclKind};
use crate::members::{Receivers, Verdict};
use crate::rewrite::{rewrite, Access};
use crate::text::line_states;

const KEYWORDS: &[&str] = &[
    "import", "from", "namespace", "as", "var", "scoped", "consumed", "property", "const", "function",
    "method", "class", "struct", "generic", "module", "enum", "interface", "wiring", "extends", "modules",
    "implements", "init", "public", "private", "protected", "static", "if", "elseif", "else", "switch",
    "default", "match", "while", "for", "in", "return", "result", "use", "break", "continue", "try", "on",
    "is", "raise", "emit", "self", "parent", "async", "resolve", "variadic", "when", "runtime", "main", "error",
    "success", "exit", "int", "float", "string", "bool", "mixed", "array", "map", "message", "void",
    "true", "false", "null", "and", "or", "not", "equal", "smaller", "greater", "nameless",
];

#[derive(Clone)]
struct Target {
    new:        String,
    member:     bool,
    /// Des fichiers portent ce nom : les segments d'import qui les
    /// désignent sont renommés avec eux.
    moves_file: bool,
}

#[derive(Default)]
pub struct Plan {
    targets:   HashMap<String, Target>,
    receivers: Receivers,
    /// Fichiers renommés avec le symbole qu'ils portent : ancien → nouveau.
    moves:     HashMap<PathBuf, PathBuf>,
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

    /// Nouveau chemin d'un fichier qui porte le nom d'un symbole renommé
    /// (`car_model.oc` déclarant `class car_model` → `CarModel.oc`).
    pub fn moved_path(&self, path: &Path) -> Option<PathBuf> {
        self.moves.get(path).cloned()
    }
}

/// Mots (identifiants) d'un fichier texte non `.oc`, avec ce fichier.
pub fn external_words(path: &Path, content: &str, out: &mut HashMap<String, PathBuf>) {
    for word in content.split(|c: char| !c.is_alphanumeric() && c != '_').filter(|w| !w.is_empty()) {
        out.entry(word.to_string()).or_insert_with(|| path.to_path_buf());
    }
}

/// Ce que le projet déclare et utilise.
#[derive(Default)]
struct Survey {
    /// Nom déclaré → (noms attendus, membre ?).
    wanted:        HashMap<String, (HashSet<String>, bool)>,
    /// Nom déclaré hors membre → fichiers qui le déclarent.
    declared_in:   HashMap<String, Vec<PathBuf>>,
    used:          HashSet<String>,
    struct_fields: HashSet<String>,
    /// Membres accédés sur un receveur de type inconnu.
    ambiguous:     HashSet<String>,
    /// Derniers segments d'import (désignent un fichier par son nom).
    import_paths:  HashSet<String>,
}

fn survey(files: &[(PathBuf, String)], receivers: &Receivers, cfg: &Config) -> Survey {
    let mut s = Survey::default();
    for (path, content) in files {
        let lines: Vec<&str> = content.lines().collect();
        let states = line_states(&lines);
        for decl in declarations(&path.to_string_lossy(), &lines, &states, cfg) {
            let target = if decl.style.matches(&decl.name) { decl.name.clone() } else { decl.style.convert(&decl.name) };
            if decl.kind != DeclKind::Member {
                s.declared_in.entry(decl.name.clone()).or_default().push(path.clone());
            }
            let entry = s.wanted.entry(decl.name).or_default();
            entry.0.insert(target);
            entry.1 |= decl.kind == DeclKind::Member;
        }
        s.struct_fields.extend(struct_fields(&lines, &states));
        rewrite(content, &mut |name, access| {
            s.used.insert(name.to_string());
            match access {
                Access::Member(m) if receivers.verdict(path, m) == Verdict::Ambiguous => { s.ambiguous.insert(name.to_string()); }
                Access::ImportPath => { s.import_paths.insert(name.to_string()); }
                _ => {}
            }
            None
        });
    }
    s
}

/// Raison d'ignorer le renommage `old` → `new`, s'il y en a une.
fn skip_reason(old: &str, new: &str, member: bool, s: &Survey, external: &HashMap<String, PathBuf>) -> Option<String> {
    if new.is_empty() {
        Some("aucun nom valide dans ce style".to_string())
    } else if KEYWORDS.contains(&new) {
        Some(format!("'{}' est un mot réservé", new))
    } else if s.used.contains(new) {
        Some(format!("'{}' est déjà utilisé dans le projet", new))
    } else if let Some(path) = external.get(old) {
        Some(format!("référencé hors des fichiers .oc ({})", path.display()))
    } else if old.ends_with("Test") && !new.ends_with("Test") {
        Some("une méthode de test ocaraunit doit se terminer par 'Test'".to_string())
    } else if member && s.ambiguous.contains(old) {
        Some("accédé sur un receveur de type inconnu (peut-être un builtin)".to_string())
    } else if !member && s.struct_fields.contains(old) {
        Some("même nom qu'un champ de struct, dont les accès '.nom' ne seraient pas renommés".to_string())
    } else {
        None
    }
}

/// Fichiers à renommer avec `old` (ceux qui le déclarent et portent son
/// nom), ou la raison de ne pas renommer.
fn file_moves(old: &str, new: &str, s: &Survey, files: &[(PathBuf, String)]) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut moves = Vec::new();
    for path in s.declared_in.get(old).into_iter().flatten() {
        if path.file_stem().and_then(|f| f.to_str()) != Some(old) || moves.iter().any(|(p, _)| p == path) { continue; }
        let dest = path.with_file_name(format!("{}.oc", new));
        if dest.exists() || files.iter().any(|(p, _)| *p == dest) {
            return Err(format!("le fichier '{}' existe déjà", dest.display()));
        }
        moves.push((path.clone(), dest));
    }
    if moves.is_empty() && s.import_paths.contains(old) {
        return Err(format!("importé par son nom de fichier, mais aucun '{}.oc' ne le déclare", old));
    }
    Ok(moves)
}

/// Plan de renommage pour l'ensemble `files` (chemin, contenu) du projet ;
/// `external` : mots des autres fichiers texte du projet (`external_words`).
pub fn plan(files: &[(PathBuf, String)], external: &HashMap<String, PathBuf>, cfg: &Config) -> Plan {
    let receivers = Receivers::collect(files);
    let s = survey(files, &receivers, cfg);
    let mut plan = Plan { receivers, ..Plan::default() };
    let mut names: Vec<_> = s.wanted.iter().collect();
    names.sort_by(|a, b| a.0.cmp(b.0));
    let mut candidates: Vec<(&String, String, bool)> = Vec::new();
    for (old, (targets, member)) in names {
        if targets.len() > 1 {
            let mut list: Vec<_> = targets.iter().cloned().collect();
            list.sort();
            plan.skipped.push(format!("'{}' non renommé : styles différents attendus selon la déclaration ({})", old, list.join(", ")));
            continue;
        }
        let Some(new) = targets.iter().next().filter(|n| *n != old) else { continue };
        match skip_reason(old, new, *member, &s, external) {
            Some(r) => plan.skipped.push(format!("'{}' non renommé en '{}' : {}", old, new, r)),
            None => candidates.push((old, new.clone(), *member)),
        }
    }
    for (old, new, member) in &candidates {
        let rivals: Vec<&str> = candidates.iter().filter(|c| c.1 == *new && c.0 != *old).map(|c| c.0.as_str()).collect();
        let moves = if rivals.is_empty() { file_moves(old, new, &s, files) } else {
            Err(format!("'{}' deviendrait aussi '{}'", rivals.join("', '"), new))
        };
        match moves {
            Err(r) => plan.skipped.push(format!("'{}' non renommé en '{}' : {}", old, new, r)),
            Ok(moves) => {
                let target = Target { new: new.clone(), member: *member, moves_file: !moves.is_empty() };
                plan.moves.extend(moves);
                plan.targets.insert((*old).clone(), target);
            }
        }
    }
    plan
}

/// Applique le plan au fichier `path` ; retourne le contenu et le nombre de
/// remplacements.
pub fn apply(path: &Path, content: &str, plan: &Plan) -> (String, usize) {
    let mut count = 0usize;
    let out = rewrite(content, &mut |name, access| {
        let target = plan.targets.get(name)?;
        let renamed = match access {
            Access::Bare => true,
            Access::ImportPath => target.moves_file,
            Access::Member(m) => target.member && plan.receivers.verdict(path, m) == Verdict::Rename,
        };
        renamed.then(|| { count += 1; target.new.clone() })
    });
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::{apply, plan};
    use crate::config::parse_config;
    use std::path::PathBuf;

    fn run(files: &[(&str, &str)]) -> (Vec<String>, Vec<String>) {
        let files: Vec<(PathBuf, String)> = files.iter().map(|(p, c)| (PathBuf::from(p), c.to_string())).collect();
        let plan = plan(&files, &Default::default(), &parse_config(""));
        (files.iter().map(|(p, c)| apply(p, c, &plan).0).collect(), plan.skipped)
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

    #[test]
    fn two_names_converging_on_one_target_are_skipped() {
        let (out, skipped) = run(&[("a.oc", "function main(): int {\n    var userID:int = 1\n    var userId:int = 2\n    return userID + userId\n}\n")]);
        assert!(out[0].contains("var userID:int = 1") && out[0].contains("var userId:int = 2"));
        assert!(skipped.iter().any(|s| s.contains("'userID'") && s.contains("deviendrait aussi")), "{:?}", skipped);
    }

    #[test]
    fn members_are_renamed_only_on_project_receivers() {
        let src = "class Window {\n    public property isOpen:bool\n    public method close(): void { self.isOpen = false }\n}\nfunction f(win:SDL, w:Window): bool {\n    return w.isOpen and win.isOpen()\n}\n";
        let (out, _) = run(&[("a.oc", src)]);
        assert!(out[0].contains("property is_open:bool") && out[0].contains("self.is_open = false"));
        assert!(out[0].contains("return w.is_open and win.isOpen()"), "{}", out[0]);
        let unknown = format!("{}function g(): bool {{\n    return make().isOpen\n}}\n", src);
        let (out, skipped) = run(&[("a.oc", &unknown)]);
        assert!(out[0].contains("property isOpen:bool"));
        assert!(skipped.iter().any(|s| s.contains("receveur de type inconnu")), "{:?}", skipped);
    }

    #[test]
    fn name_shared_with_a_bare_struct_field_is_skipped() {
        let (out, skipped) = run(&[("a.oc", "struct User {\n    userName:string\n}\nfunction main(): int {\n    var userName:string = \"x\"\n    var u:User = use User(userName)\n    return u.userName.len()\n}\n")]);
        assert!(out[0].contains("    userName:string\n") && out[0].contains("var userName:string"));
        assert!(skipped.iter().any(|s| s.contains("champ de struct")), "{:?}", skipped);
    }

    #[test]
    fn imported_function_moves_its_file_and_import_keeps_its_comment() {
        let files: Vec<(PathBuf, String)> = vec![
            (PathBuf::from("lib/do_stuff.oc"), "function do_stuff(): void {\n}\n".to_string()),
            (PathBuf::from("main.oc"), "import lib.do_stuff // aide\nimport do_stuff from \"./lib/do_stuff\" as alias_x\nfunction main(): int {\n    do_stuff()\n    return 0\n}\n".to_string()),
        ];
        let plan = plan(&files, &Default::default(), &parse_config(""));
        assert_eq!(plan.moved_path(&files[0].0), Some(PathBuf::from("lib/doStuff.oc")));
        let main = apply(&files[1].0, &files[1].1, &plan).0;
        assert!(main.starts_with("import lib.doStuff // aide\nimport doStuff from \"./lib/doStuff\" as alias_x\n"), "{}", main);
    }

    #[test]
    fn import_of_a_file_not_named_after_the_symbol_blocks_the_rename() {
        let (out, skipped) = run(&[
            ("lib/tools.oc", "function do_stuff(): void {\n}\n"),
            ("main.oc", "import lib.do_stuff\nfunction main(): int {\n    return 0\n}\n"),
        ]);
        assert!(out[0].contains("function do_stuff()") && out[1].starts_with("import lib.do_stuff\n"));
        assert!(skipped.iter().any(|s| s.contains("nom de fichier")), "{:?}", skipped);
    }

    #[test]
    fn reserved_when_and_quoted_brace() {
        let (out, skipped) = run(&[("a.oc", "function When(): int { return 1 }\nfunction f(): string {\n    return '{'\n}\nconst max_size = 3\n")]);
        assert!(out[0].contains("function When()") && skipped.iter().any(|s| s.contains("mot réservé")), "{:?}", skipped);
        assert!(out[0].contains("const MAX_SIZE = 3"), "{}", out[0]);
    }
}
