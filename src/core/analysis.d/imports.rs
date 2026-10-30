//! Vérification, chargement et fusion des imports dans le programme
//! d'entrée.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::core::alias_resolve::{compute_aliases, resolve_aliases};
use crate::core::diagnostics::Diagnostic;
use crate::core::runtime_expand::update_program_spans_with_file;
use crate::parsing::ast::{ClassDecl, ImportDecl, InterfaceDecl, Program, WiringDecl};
use crate::parsing::{lexer::Lexer, parser::Parser};

/// Les modules `ocara.*` sont livrés avec le runtime.
pub const OCARA_BUILTINS: &[&str] = &[
    "IO", "Math", "String", "Array", "Map", "JSON", "Tauri", "UI", "UIEvent", "UIComponent", "SDL", "Image",
    "Convert", "System", "Regex", "HTTPRequest", "HTTPResponse", "HTTPServer", "HTTPServerRequest", "HTTPServerSession", "SQLite", "MySQL", "MariaDB", "DotEnv", "YAML", "Thread", "Mutex",
    "DateTime", "Date", "Time", "UnitTest", "HTMLComponent", "HTML",
    "File", "Directory", "Exception", "FileException", "DirectoryException", "IOException", "SystemException",
    "ArrayException", "MapException", "MathException", "ConvertException", "RegexException",
    "DateTimeException", "DateException", "TimeException",
    "ThreadException", "MutexException",
    "UnitTestException", "HTTPServerException", "SQLiteException", "MySQLException", "MariaDBException", "DotEnvException", "YAMLException",
    "SDLException", "TauriException", "ImageException", "AndroidException", "UIException",
    "AndroidPermissions", "AndroidNotifications", "AndroidSensors", "AndroidFiles", "AndroidCamera", "AndroidMicrophone", "AndroidAudio", "AndroidUI",
];

/// (import, répertoire du fichier parent, namespace du fichier parent)
type Pending = Vec<(ImportDecl, PathBuf, Option<String>)>;

/// Import implicite de chaque `wiring` d'une interface qui vient d'entrer
/// dans le programme (voir docs/roadmap.d/langage-interface-wiring.md,
/// « Import implicite »). Une classe cible déclarée dans le fichier qui
/// vient d'être chargé (`local_pool`) est rapatriée directement : il n'y a
/// pas de fichier séparé à charger.
fn enqueue_wiring_imports(
    wirings: &[WiringDecl],
    parent_dir: &Path,
    parent_namespace: &Option<String>,
    imports_to_process: &mut Pending,
    program_classes: &mut Vec<ClassDecl>,
    local_pool: &[ClassDecl],
) {
    for w in wirings {
        let target = w.simple_name();
        if program_classes.iter().any(|c| c.name == target) {
            continue;
        }
        if let Some(local_cls) = local_pool.iter().find(|c| c.name == target) {
            program_classes.push(local_cls.clone());
            continue;
        }
        let virtual_imp = ImportDecl {
            path: vec![target.to_string()],
            alias: None,
            file_path: Some(w.path.join("/")),
            span: w.span.clone(),
        };
        imports_to_process.push((virtual_imp, parent_dir.to_path_buf(), parent_namespace.clone()));
    }
}

/// Chaque import doit désigner un module builtin ou un fichier `.oc` existant.
pub fn check_imports(program: &Program, source_dir: &Path, input: &Path) -> Result<(), Diagnostic> {
    for imp in &program.imports {
        if imp.path.first().map(|s| s.as_str()) == Some("ocara") {
            let last = imp.path.last().map(|s| s.as_str()).unwrap_or("");
            if last != "*" && !OCARA_BUILTINS.contains(&last) {
                return Err(Diagnostic::at(input, &imp.span,
                    format!("unknown builtin module: `{}` (available modules: {})", imp.path.join("."), OCARA_BUILTINS.join(", "))));
            }
            continue;
        }

        // Même résolution que `load_imports` : namespace du fichier d'entrée
        // d'abord (`namespace configs` + `import routes.Home` →
        // `configs/routes/Home.oc`), puis racine.
        if let Some(file_path_str) = &imp.file_path {
            let file_path = resolve_import_path(file_path_str, source_dir, &program.namespace, source_dir);
            if !file_path.exists() {
                return Err(Diagnostic::at(input, &imp.span,
                    format!("file not found: `{}` (expected file: {})", file_path_str, file_path.display())));
            }
            continue;
        }

        let file_path = resolve_import_path(&imp.path.join("/"), source_dir, &program.namespace, source_dir);
        if !file_path.exists() {
            return Err(Diagnostic::at(input, &imp.span,
                format!("module not found: `{}` (expected file: {})", imp.path.join("."), file_path.display())));
        }
    }
    Ok(())
}

/// Import namespace (`import a.b.C`) → import virtuel `C` depuis `a/b/C`.
pub(super) fn virtual_from(imp: &ImportDecl) -> ImportDecl {
    ImportDecl {
        path: vec![imp.path.last().cloned().unwrap_or_default()],
        alias: imp.alias.clone(),
        file_path: Some(imp.path.join("/")),
        span: imp.span.clone(),
    }
}

pub fn resolve_import_path(file_path_str: &str, parent_dir: &Path, parent_namespace: &Option<String>, source_dir: &Path) -> PathBuf {
    let clean_path = file_path_str.trim_end_matches(".oc");
    if clean_path.starts_with("../") || clean_path.starts_with("./") {
        let mut file_path = parent_dir.join(clean_path);
        if file_path.extension().is_none() {
            file_path.set_extension("oc");
        }
        return file_path;
    }
    // Namespace du fichier parent d'abord, puis racine.
    if let Some(ns) = parent_namespace.as_deref().filter(|ns| *ns != ".") {
        let in_namespace = source_dir.join(ns.replace('.', "/")).join(clean_path).with_extension("oc");
        if in_namespace.exists() {
            return in_namespace;
        }
    }
    source_dir.join(clean_path).with_extension("oc")
}

/// Fichier importé analysé en mode tolérant : programme partiel et erreurs
/// de syntaxe (le lexing reste bloquant).
fn parse_module_recovering(file_path: &Path) -> Result<(Program, Vec<Diagnostic>), Diagnostic> {
    let src = crate::core::source::read(file_path)
        .map_err(|e| Diagnostic::error(file_path, 1, 1, format!("reading file '{}': {}", file_path.display(), e)))?;
    let tokens = Lexer::new(&src).tokenize()
        .map_err(|e| Diagnostic::error(file_path, 0, 0, format!("{}", e)))?;
    let (program, errors) = crate::parsing::build::parsing_file(&file_path.to_string_lossy(), || Parser::new(tokens).parse_program_recovering());
    let syntax = errors.into_iter().map(|e| Diagnostic::error(file_path, e.span.line, e.span.col, e.message)).collect();
    Ok((program, syntax))
}

pub(super) fn parse_module(file_path: &Path) -> Result<Program, Diagnostic> {
    let src = crate::core::source::read(file_path)
        .map_err(|e| Diagnostic::error(file_path, 1, 1, format!("reading file '{}': {}", file_path.display(), e)))?;
    let tokens = Lexer::new(&src).tokenize()
        .map_err(|e| Diagnostic::error(file_path, 0, 0, format!("{}", e)))?;
    crate::parsing::build::parsing_file(&file_path.to_string_lossy(), || Parser::new(tokens).parse_program())
        .map_err(|e| Diagnostic::error(file_path, e.span.line, e.span.col, e.message))
}

/// Charge récursivement les fichiers importés et fusionne dans `program` ce
/// que chaque import demande. Dédupliqué par (fichier, symbole demandé) :
/// deux imports de symboles différents d'un même fichier sont tous deux
/// traités ; un fichier n'est parsé qu'une fois.
pub fn load_imports(
    program: &mut Program,
    source_dir: &Path,
    input: &Path,
    all_interfaces: &HashMap<String, InterfaceDecl>,
    homonyms: &super::homonyms::Homonyms,
    tolerant: bool,
) -> Result<Vec<Diagnostic>, Diagnostic> {
    let mut syntax = Vec::new();
    let mut processed: HashSet<(PathBuf, String)> = HashSet::new();
    let mut parsed_files_cache: HashMap<PathBuf, Program> = HashMap::new();
    let mut imports_to_process: Pending = Vec::new();
    let main_namespace = program.namespace.clone();
    // Fichier d'entrée atteint par un cycle (interface → `wiring` → classe
    // qui importe l'interface) : ses déclarations sont déjà dans `program`.
    let entry = input.canonicalize().unwrap_or_else(|_| input.to_path_buf());

    for imp in program.imports.iter().filter(|imp| imp.file_path.is_some()) {
        imports_to_process.push((imp.clone(), source_dir.to_path_buf(), main_namespace.clone()));
    }
    for imp in program.imports.iter().filter(|imp| imp.file_path.is_none() && imp.path.first().map(|s| s.as_str()) != Some("ocara")) {
        imports_to_process.push((virtual_from(imp), source_dir.to_path_buf(), main_namespace.clone()));
    }
    let interfaces = program.interfaces.clone();
    for iface in &interfaces {
        enqueue_wiring_imports(&iface.wirings, source_dir, &main_namespace, &mut imports_to_process, &mut program.classes, &[]);
    }
    // `wiring` d'une classe de test : import implicite, comme pour une
    // interface (voir core::test_wiring).
    let test_wirings = crate::core::test_wiring::targets(program);
    enqueue_wiring_imports(&test_wirings, source_dir, &main_namespace, &mut imports_to_process, &mut program.classes, &[]);

    while !imports_to_process.is_empty() {
        let (imp, parent_dir, parent_namespace) = imports_to_process.remove(0);
        let file_path = resolve_import_path(imp.file_path.as_ref().unwrap(), &parent_dir, &parent_namespace, source_dir);

        let canonical_path = file_path.canonicalize().unwrap_or(file_path.clone());
        let is_wildcard = imp.path.first().is_some_and(|s| s == "*");
        let requested_key = if is_wildcard { "*".to_string() } else { imp.path.first().cloned().unwrap_or_default() };
        if canonical_path == entry || !processed.insert((canonical_path.clone(), requested_key)) {
            continue;
        }

        let current_file_dir = file_path.parent().unwrap_or(&parent_dir).to_path_buf();
        let mut mod_prog = match parsed_files_cache.get(&canonical_path) {
            Some(cached) => cached.clone(),
            None => {
                let parsed = if tolerant {
                    let (parsed, errors) = parse_module_recovering(&file_path)?;
                    syntax.extend(errors);
                    parsed
                } else {
                    parse_module(&file_path)?
                };
                parsed_files_cache.insert(canonical_path.clone(), parsed.clone());
                parsed
            }
        };

        update_program_spans_with_file(&mut mod_prog, &file_path.to_string_lossy());
        let loaded_namespace = mod_prog.namespace.clone();

        // Alias écrits dans CE fichier, sur ses propres imports (voir
        // core::alias_resolve) : les symboles fusionnés gardent leur vrai nom.
        let mod_file_aliases = compute_aliases(&mod_prog.imports, all_interfaces, &file_path)?;
        resolve_aliases(&mut mod_prog, &mod_file_aliases);
        homonyms.apply(&mut mod_prog, &file_path);
        crate::core::test_wiring::check_imported(&mod_prog, &file_path)?;

        if is_wildcard {
            for iface in &mod_prog.interfaces {
                enqueue_wiring_imports(&iface.wirings, &current_file_dir, &loaded_namespace, &mut imports_to_process, &mut program.classes, &mod_prog.classes);
            }
            program.classes.extend(mod_prog.classes);
            program.interfaces.extend(mod_prog.interfaces);
            // Le point d'entrée d'un fichier importé n'est pas importé : il
            // doublerait celui du programme (un fichier de tests qui importe
            // le `main.oc` d'un projet).
            program.functions.extend(mod_prog.functions.into_iter().filter(|f| f.name != "main"));
            program.consts.extend(mod_prog.consts);
            program.modules.extend(mod_prog.modules);
            program.generics.extend(mod_prog.generics);
        } else {
            // Symbole homonyme : demandé sous son nom interne dans ce fichier.
            let mut imp = imp;
            if let Some(int) = imp.path.first().and_then(|n| homonyms.internal(&canonical_path, n)) {
                imp.path[0] = int;
            }
            merge_requested(program, &mod_prog, &imp, &file_path, &current_file_dir, &loaded_namespace, &mut imports_to_process, input)?;
        }

        for new_imp in mod_prog.imports {
            if new_imp.path.first().map(|s| s.as_str()) == Some("ocara") {
                if !program.imports.iter().any(|i| i.path == new_imp.path) {
                    program.imports.push(new_imp);
                }
                continue;
            }
            if !program.imports.iter().any(|i| i.path == new_imp.path && i.file_path == new_imp.file_path) {
                program.imports.push(new_imp.clone());
            }
            if new_imp.file_path.is_some() {
                imports_to_process.push((new_imp, current_file_dir.clone(), loaded_namespace.clone()));
            } else {
                imports_to_process.push((virtual_from(&new_imp), source_dir.to_path_buf(), loaded_namespace.clone()));
            }
        }
    }

    dedup(program);
    Ok(syntax)
}

/// Import sélectif : le symbole demandé (classe → generic → interface →
/// module → fonction), plus les constantes de son fichier et les interfaces
/// implémentées par une classe importée.
#[allow(clippy::too_many_arguments)]
fn merge_requested(
    program: &mut Program,
    mod_prog: &Program,
    imp: &ImportDecl,
    file_path: &Path,
    current_file_dir: &Path,
    loaded_namespace: &Option<String>,
    imports_to_process: &mut Pending,
    input: &Path,
) -> Result<(), Diagnostic> {
    let requested_name = imp.path.first().cloned().unwrap_or_default();

    for c in &mod_prog.consts {
        if !program.consts.iter().any(|existing| existing.name == c.name) {
            program.consts.push(c.clone());
        }
    }

    if let Some(cls) = mod_prog.classes.iter().find(|c| c.name == requested_name).cloned() {
        merge_class_with_deps(program, mod_prog, cls, current_file_dir, loaded_namespace, imports_to_process);
    } else if let Some(generic_item) = mod_prog.generics.iter().find(|g| g.name == requested_name).cloned() {
        program.generics.push(generic_item);
    } else if let Some(iface) = mod_prog.interfaces.iter().find(|i| i.name == requested_name).cloned() {
        enqueue_wiring_imports(&iface.wirings, current_file_dir, loaded_namespace, imports_to_process, &mut program.classes, &mod_prog.classes);
        program.interfaces.push(iface);
    } else if let Some(module) = mod_prog.modules.iter().find(|m| m.name == requested_name).cloned() {
        program.modules.push(module);
    } else if requested_name == "main" && mod_prog.functions.iter().any(|f| f.name == "main") {
        return Err(Diagnostic::at(input, &imp.span,
            format!("'main' is the entry point of '{}' and cannot be imported — import the classes and functions you need by name", file_path.display())));
    } else if let Some(func) = mod_prog.functions.iter().find(|f| f.name == requested_name).cloned() {
        program.functions.push(func);
    } else {
        let declared: Vec<&str> = mod_prog.classes.iter().map(|c| c.name.as_str())
            .chain(mod_prog.generics.iter().map(|g| g.name.as_str()))
            .chain(mod_prog.interfaces.iter().map(|i| i.name.as_str()))
            .chain(mod_prog.modules.iter().map(|m| m.name.as_str()))
            .chain(mod_prog.functions.iter().map(|f| f.name.as_str()))
            .collect();
        let hint = if declared.is_empty() {
            "it declares nothing importable".to_string()
        } else {
            format!("it declares: {}", declared.join(", "))
        };
        return Err(Diagnostic::at(input, &imp.span,
            format!("'{}' not found in file '{}' — {}", requested_name, file_path.display(), hint)));
    }
    Ok(())
}

/// Classe importée sélectivement, avec ses dépendances déclarées dans le même
/// fichier : interfaces implémentées, `modules` et classe parente (elle-même
/// avec ses dépendances) — voir EBNF « Chargement récursif ». Celles d'un
/// autre fichier arrivent par les imports de ce fichier.
fn merge_class_with_deps(
    program: &mut Program,
    mod_prog: &Program,
    cls: ClassDecl,
    current_file_dir: &Path,
    loaded_namespace: &Option<String>,
    imports_to_process: &mut Pending,
) {
    if program.classes.iter().any(|c| c.name == cls.name) {
        return;
    }
    for iface_name in &cls.implements {
        if program.interfaces.iter().any(|i| &i.name == iface_name) {
            continue;
        }
        if let Some(iface) = mod_prog.interfaces.iter().find(|i| &i.name == iface_name).cloned() {
            enqueue_wiring_imports(&iface.wirings, current_file_dir, loaded_namespace, imports_to_process, &mut program.classes, &mod_prog.classes);
            program.interfaces.push(iface);
        }
    }
    for module_name in &cls.modules {
        if program.modules.iter().any(|m| &m.name == module_name) {
            continue;
        }
        if let Some(module) = mod_prog.modules.iter().find(|m| &m.name == module_name).cloned() {
            program.modules.push(module);
        }
    }
    let parent = cls.extends.as_ref()
        .and_then(|name| mod_prog.classes.iter().find(|c| &c.name == name).cloned());
    program.classes.push(cls);
    if let Some(parent) = parent {
        merge_class_with_deps(program, mod_prog, parent, current_file_dir, loaded_namespace, imports_to_process);
    }
}

/// Un module peut introduire des doublons (classe rapatriée puis
/// refusionnée) : seule une déclaration identique est retirée — deux
/// déclarations différentes de même nom restent, signalées par la sema
/// (E03, `crate::sema::duplicates`).
fn dedup(program: &mut Program) {
    retain_distinct(&mut program.classes);
    retain_distinct(&mut program.functions);
    retain_distinct(&mut program.consts);
}

fn retain_distinct<T: PartialEq + Clone>(items: &mut Vec<T>) {
    let mut kept: Vec<T> = Vec::with_capacity(items.len());
    for item in items.drain(..) {
        if !kept.contains(&item) {
            kept.push(item);
        }
    }
    *items = kept;
}
