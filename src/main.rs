mod ast;
mod builtins;
mod codegen;
mod diagnostic;
mod error;
mod ir;
mod lexer;
mod lower;
mod parser;
mod sema;
mod token;

use std::fs;
use std::path::PathBuf;

use codegen::emit::CraneliftEmitter;
use codegen::link::link;
use lexer::Lexer;
use lower::builder::lower_program;
use parser::Parser;
use sema::symbols::SymbolTable;
use sema::typecheck::TypeChecker;

// ─────────────────────────────────────────────────────────────────────────────
// CLI minimale
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct CliArgs {
    input:   PathBuf,
    output:  PathBuf,
    /// true = afficher les tokens + HIR sans compiler
    dump:    bool,
    /// true = s'arrêter après l'analyse sémantique
    check:   bool,
    /// true = produire le fichier .o mais ne pas linker
    no_link: bool,
    /// true = strip les symboles du binaire produit (via le linker)
    release: bool,
    /// Répertoire racine pour la résolution des imports (défaut : répertoire du fichier d'entrée)
    src_dir: Option<PathBuf>,
}

fn print_help() {
    println!("Ocara — Object Code Abstraction Runtime Architecture v{}", env!("CARGO_PKG_VERSION"));
    println!("Un langage de programmation simple avec un compilateur écrit en Rust.");
    println!("Auteur : David Lhoumaud");
    println!();
    println!("Usage :");
    println!("  ocara <fichier.oc> [options]");
    println!();
    println!("Options :");
    println!("  -o <sortie>   Fichier de sortie (défaut : out)");
    println!("  --src <dir>   Répertoire racine pour la résolution des imports");
    println!("  --check       Analyse sémantique uniquement, sans compilation");
    println!("  --dump        Affiche les tokens et l'AST");
    println!("  --no-link     Produit le fichier .o sans linker");
    println!("  -h, --help    Affiche cette aide");
    println!();
    println!("Exemples :");
    println!("  ocara main.oc -o ./mon_programme");
    println!("  ocara main.oc --check");
    println!("  ocara tests/mainTest.oc --src .");
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();

    // Aide explicite ou aucun argument
    if args.len() < 2 || args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        std::process::exit(0);
    }

    let mut input   = PathBuf::from("test.oc");
    let mut output  = PathBuf::from("out");
    let mut dump    = false;
    let mut check   = false;
    let mut no_link = false;
    let mut release = false;
    let mut src_dir = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dump"    => dump    = true,
            "--check"   => check   = true,
            "--no-link" => no_link = true,
            "--release" => release = true,
            "-o" if i + 1 < args.len() => {
                output = PathBuf::from(&args[i + 1]);
                i += 1;
            }
            "--src" if i + 1 < args.len() => {
                src_dir = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            arg => {
                if !arg.starts_with('-') {
                    input = PathBuf::from(arg);
                }
            }
        }
        i += 1;
    }
    CliArgs { input, output, dump, check, no_link, release, src_dir }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pipeline de compilation complet
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    let args = parse_args();

    // ── 1. Lecture du source ──────────────────────────────────────────────────
    let source = match fs::read_to_string(&args.input) {
        Ok(s) => s,
        Err(e) => {
            diagnostic::print_error(&args.input, 0, 0, &format!("cannot read '{}': {}", args.input.display(), e));
            std::process::exit(1);
        }
    };

    // ── 2. Lexing ─────────────────────────────────────────────────────────────
    let tokens = match Lexer::new(&source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            use crate::error::LexError;
            let (line, col) = match &e {
                LexError::UnexpectedChar(_, s)    => (s.line, s.col),
                LexError::UnterminatedString(s)   => (s.line, s.col),
                LexError::InvalidEscape(_, s)     => (s.line, s.col),
                LexError::IntegerOverflow(_, s)   => (s.line, s.col),
            };
            let msg = match &e {
                LexError::UnexpectedChar(ch, _)    => format!("unexpected character '{}'", ch),
                LexError::UnterminatedString(_)    => "unterminated string".into(),
                LexError::InvalidEscape(ch, _)     => format!("invalid escape sequence '\\{}'", ch),
                LexError::IntegerOverflow(raw, _)  => format!("integer too large: {}", raw),
            };
            diagnostic::print_error(&args.input, line, col, &msg);
            std::process::exit(1);
        }
    };

    if args.dump {
        let non_eof: Vec<_> = tokens.iter()
            .filter(|t| t.kind != token::TokenKind::Eof)
            .collect();
        println!("=== TOKENS ({}) ===", non_eof.len());
        for tok in &non_eof { println!("{}", tok); }
        println!();
    }

    // ── 3. Parsing ────────────────────────────────────────────────────────────
    let mut program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => {
            diagnostic::print_error(&args.input, e.span.line, e.span.col, &e.message);
            std::process::exit(1);
        }
    };

    if args.dump {
        println!("=== AST ===");
        println!("{:#?}", program);
        println!();
    }

    // ── 4. Vérification des imports non-builtins ──────────────────────────────
    // Les modules `ocara.*` sont builtins (livrés avec le runtime).
    // Tout autre import doit pointer vers un fichier .oc existant.
    const OCARA_BUILTINS: &[&str] = &[
        "IO", "Math", "String", "Array", "Map", "JSON",
        "Convert", "System", "Regex", "HTTPRequest", "HTTPServer", "Thread", "Mutex",
        "DateTime", "Date", "Time", "UnitTest", "HTMLComponent", "HTML",
        "File", "Directory", "Exception", "FileException", "DirectoryException", "IOException", "SystemException",
        "ArrayException", "MapException", "MathException", "ConvertException", "RegexException",
        "DateTimeException", "DateException", "TimeException",
        "ThreadException", "MutexException",
        "UnitTestException",
    ];
    // Répertoire de base pour la résolution des imports
    let source_dir = args.src_dir.as_ref()
        .map(|p| p.as_path())
        .unwrap_or_else(|| args.input.parent().unwrap_or_else(|| std::path::Path::new(".")));
    
    // Séparer les imports en deux catégories
    let module_imports: Vec<crate::ast::ImportDecl> = program.imports.iter()
        .filter(|imp| imp.file_path.is_none() && imp.path.first().map(|s| s.as_str()) != Some("ocara"))
        .cloned()
        .collect();
    
    let file_imports: Vec<crate::ast::ImportDecl> = program.imports.iter()
        .filter(|imp| imp.file_path.is_some())
        .cloned()
        .collect();

    // Vérification des imports
    for imp in &program.imports {
        if imp.path.first().map(|s| s.as_str()) == Some("ocara") {
            // Import builtin : vérifier que le module existe dans le runtime
            let last = imp.path.last().map(|s| s.as_str()).unwrap_or("");
            if last != "*" && !OCARA_BUILTINS.contains(&last) {
                let name = imp.path.join(".");
                diagnostic::print_error(&args.input, imp.span.line, imp.span.col,
                    &format!("unknown builtin module: `{}` (available modules: {})", name, OCARA_BUILTINS.join(", ")));
                std::process::exit(1);
            }
            continue;
        }
        
        // Import depuis un fichier (nouveau format "from")
        if let Some(file_path_str) = &imp.file_path {
            let mut file_path = source_dir.to_path_buf();
            // Support des chemins relatifs avec ../
            let clean_path = file_path_str.trim_end_matches(".oc");
            file_path.push(clean_path);
            if !file_path.extension().is_some() {
                file_path.set_extension("oc");
            }
            
            if !file_path.exists() {
                diagnostic::print_error(&args.input, imp.span.line, imp.span.col,
                    &format!("file not found: `{}` (expected file: {})", file_path_str, file_path.display()));
                std::process::exit(1);
            }
            continue;
        }
        
        // Import utilisateur (ancien format) : vérifier que le fichier .oc existe
        let mut file_path = source_dir.to_path_buf();
        for segment in &imp.path {
            file_path.push(segment);
        }
        file_path.set_extension("oc");
        if !file_path.exists() {
            let name = imp.path.join(".");
            diagnostic::print_error(&args.input, imp.span.line, imp.span.col,
                &format!("module not found: `{}` (expected file: {})", name, file_path.display()));
            std::process::exit(1);
        }
    }

    // ── 4a. Chargement et fusion des imports (nouveau + ancien format) ───────
    let mut processed_files: std::collections::HashSet<std::path::PathBuf> = std::collections::HashSet::new();
    
    // (ImportDecl, répertoire du fichier parent, namespace du fichier parent)
    let mut imports_to_process: Vec<(crate::ast::ImportDecl, std::path::PathBuf, Option<String>)> = Vec::new();
    
    // Namespace du fichier principal
    let main_namespace = program.namespace.clone();
    
    // Ajouter les imports "from" (nouveau format)
    for imp in &file_imports {
        imports_to_process.push((imp.clone(), source_dir.to_path_buf(), main_namespace.clone()));
    }
    
    // Ajouter les imports namespace (ancien format) - créer un ImportDecl virtuel avec file_path
    for imp in &module_imports {
        // Si l'import n'a qu'un seul segment (ex: UIComponent) et qu'on est dans un namespace,
        // essayer d'abord dans le namespace courant
        let file_path_str = if imp.path.len() == 1 && main_namespace.is_some() && main_namespace.as_deref() != Some(".") {
            // Essayer namespace/Symbol (ex: classes/UIComponent)
            let ns = main_namespace.as_ref().unwrap();
            format!("{}/{}", ns, imp.path[0])
        } else {
            // Chemin complet (ex: classes.Button → classes/Button)
            imp.path.join("/")
        };
        
        // Le dernier segment est le nom du symbole à importer
        let symbol_name = imp.path.last().cloned().unwrap_or_default();
        
        // Créer un import virtuel avec file_path pour le traiter comme un import "from"
        let virtual_imp = crate::ast::ImportDecl {
            path: vec![symbol_name], // importer ce symbole spécifique
            alias: imp.alias.clone(),
            file_path: Some(file_path_str),
            span: imp.span.clone(),
        };
        imports_to_process.push((virtual_imp, source_dir.to_path_buf(), main_namespace.clone()));
    }
    
    while !imports_to_process.is_empty() {
        let (imp, parent_dir, _parent_namespace) = imports_to_process.remove(0);
        let file_path_str = imp.file_path.as_ref().unwrap();
        
        // Résoudre le chemin depuis le répertoire parent
        let clean_path = file_path_str.trim_end_matches(".oc");
        let mut file_path = if clean_path.starts_with("../") || clean_path.starts_with("./") {
            // Chemin relatif : résoudre depuis le répertoire parent
            parent_dir.join(clean_path)
        } else {
            // Chemin absolu ou depuis source_dir
            source_dir.join(clean_path)
        };
        
        if !file_path.extension().is_some() {
            file_path.set_extension("oc");
        }
        
        // Si le fichier n'existe pas et que le chemin pourrait être dans un namespace,
        // essayer un fallback vers la racine (sans le namespace)
        if !file_path.exists() && file_path_str.contains('/') {
            let parts: Vec<&str> = file_path_str.split('/').collect();
            if parts.len() == 2 {
                // Essayer juste le nom du symbole (ex: classes/UIComponent → UIComponent)
                let fallback_path = source_dir.join(parts[1]).with_extension("oc");
                if fallback_path.exists() {
                    file_path = fallback_path;
                }
            }
        }
        
        // Éviter de traiter le même fichier plusieurs fois
        let canonical_path = file_path.canonicalize().unwrap_or(file_path.clone());
        if processed_files.contains(&canonical_path) {
            continue;
        }
        processed_files.insert(canonical_path.clone());
        
        // Le répertoire parent pour les imports de ce fichier
        let current_file_dir = file_path.parent().unwrap_or(&parent_dir).to_path_buf();

        let mod_src = match fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(e) => {
                diagnostic::print_error(&file_path, 0, 0, &format!("reading file '{}': {}", file_path.display(), e));
                std::process::exit(1);
            }
        };
        let mod_tokens = match Lexer::new(&mod_src).tokenize() {
            Ok(t) => t,
            Err(e) => {
                diagnostic::print_error(&file_path, 0, 0, &format!("{}", e));
                std::process::exit(1);
            }
        };
        let mod_prog = match Parser::new(mod_tokens).parse_program() {
            Ok(p) => p,
            Err(e) => {
                diagnostic::print_error(&file_path, e.span.line, e.span.col, &e.message);
                std::process::exit(1);
            }
        };

        // Extraire ce qui est demandé
        let is_wildcard = imp.path.first().map(|s| s == "*").unwrap_or(false);
        
        if is_wildcard {
            // import * from "file" → tout importer
            program.classes.extend(mod_prog.classes);
            program.interfaces.extend(mod_prog.interfaces);
            program.functions.extend(mod_prog.functions);
            program.consts.extend(mod_prog.consts);
            program.modules.extend(mod_prog.modules);
        } else {
            // import Circle from "file" → importer seulement Circle
            let requested_name = imp.path.first().cloned().unwrap_or_default();
            let final_name = imp.alias.as_ref().cloned().unwrap_or(requested_name.clone());
            
            // Chercher la classe
            if let Some(mut cls) = mod_prog.classes.iter().find(|c| c.name == requested_name).cloned() {
                cls.name = final_name.clone();
                program.classes.push(cls);
            }
            // Chercher l'interface
            else if let Some(mut iface) = mod_prog.interfaces.iter().find(|i| i.name == requested_name).cloned() {
                iface.name = final_name.clone();
                program.interfaces.push(iface);
            }
            // Chercher le module
            else if let Some(mut module) = mod_prog.modules.iter().find(|m| m.name == requested_name).cloned() {
                module.name = final_name.clone();
                program.modules.push(module);
            }
            // Chercher la fonction
            else if let Some(mut func) = mod_prog.functions.iter().find(|f| f.name == requested_name).cloned() {
                func.name = final_name.clone();
                program.functions.push(func);
            }
            else {
                diagnostic::print_error(&args.input, imp.span.line, imp.span.col,
                    &format!("'{}' not found in file '{}'", requested_name, file_path_str));
                std::process::exit(1);
            }
        }
        
        // Récupérer le namespace du fichier chargé
        let loaded_namespace = mod_prog.namespace.clone();
        
        // Ajouter les imports du module chargé pour traitement récursif
        for new_imp in mod_prog.imports {
            // Skip les imports ocara.* (builtins)
            if new_imp.path.first().map(|s| s.as_str()) == Some("ocara") {
                if !program.imports.iter().any(|i| i.path == new_imp.path) {
                    program.imports.push(new_imp.clone());
                }
                continue;
            }
            
            // Ajouter à la liste globale si pas déjà présent
            if !program.imports.iter().any(|i| i.path == new_imp.path && i.file_path == new_imp.file_path) {
                program.imports.push(new_imp.clone());
            }
            
            // Ajouter à la file de traitement récursif
            if new_imp.file_path.is_some() {
                // Import "from" - ajouter tel quel
                imports_to_process.push((new_imp, current_file_dir.clone(), loaded_namespace.clone()));
            } else {
                // Import namespace - convertir en import virtuel "from"
                // Si l'import n'a qu'un seul segment et qu'on est dans un namespace,
                // essayer d'abord dans le namespace courant
                let file_path_str = if new_imp.path.len() == 1 && loaded_namespace.is_some() && loaded_namespace.as_deref() != Some(".") {
                    // Essayer namespace/Symbol (ex: classes/UIComponent)
                    let ns = loaded_namespace.as_ref().unwrap();
                    format!("{}/{}", ns, new_imp.path[0])
                } else {
                    // Chemin complet (ex: classes.Button → classes/Button)
                    new_imp.path.join("/")
                };
                let symbol_name = new_imp.path.last().cloned().unwrap_or_default();
                
                let virtual_imp = crate::ast::ImportDecl {
                    path: vec![symbol_name],
                    alias: new_imp.alias.clone(),
                    file_path: Some(file_path_str),
                    span: new_imp.span.clone(),
                };
                imports_to_process.push((virtual_imp, source_dir.to_path_buf(), loaded_namespace.clone()));
            }
        }
    }

    // ── 4b. Chargement et fusion des modules utilisateur (ancien format) ─────
    for imp in &module_imports {
        let mut file_path = source_dir.to_path_buf();
        for segment in &imp.path { file_path.push(segment); }
        file_path.set_extension("oc");

        let mod_src = match fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(e) => {
                diagnostic::print_error(&file_path, 0, 0, &format!("reading module '{}': {}", file_path.display(), e));
                std::process::exit(1);
            }
        };
        let mod_tokens = match Lexer::new(&mod_src).tokenize() {
            Ok(t) => t,
            Err(e) => {
                diagnostic::print_error(&file_path, 0, 0, &format!("{}", e));
                std::process::exit(1);
            }
        };
        let mut mod_prog = match Parser::new(mod_tokens).parse_program() {
            Ok(p) => p,
            Err(e) => {
                diagnostic::print_error(&file_path, e.span.line, e.span.col, &e.message);
                std::process::exit(1);
            }
        };

        // Renommage via alias : la classe dont le nom = dernier segment → alias
        if let Some(alias) = &imp.alias {
            let class_name = imp.path.last().cloned().unwrap_or_default();
            for cls in &mut mod_prog.classes {
                if cls.name == class_name {
                    cls.name = alias.clone();
                }
            }
        }

        // Fusion dans le programme principal
        program.classes.extend(mod_prog.classes);
        program.functions.extend(mod_prog.functions);
        program.consts.extend(mod_prog.consts);
        // Ajouter les imports du module (ex: ocara.IO) s'ils ne sont pas déjà présents
        for new_imp in mod_prog.imports {
            if !program.imports.iter().any(|i| i.path == new_imp.path) {
                program.imports.push(new_imp);
            }
        }
    }

    // ── 4b. Déduplication (modules peuvent introduire des doublons) ───────────
    {
        let mut seen = std::collections::HashSet::new();
        program.classes.retain(|c| seen.insert(c.name.clone()));
    }
    {
        let mut seen = std::collections::HashSet::new();
        program.functions.retain(|f| seen.insert(f.name.clone()));
    }
    {
        let mut seen = std::collections::HashSet::new();
        program.consts.retain(|c| seen.insert(c.name.clone()));
    }

    // ── 4c. Construction de la table des symboles ─────────────────────────────
    let mut symbols = SymbolTable::new();
    for decl in &program.imports    { symbols.register_import(decl); }
    for decl in &program.consts     { symbols.register_const(decl); }
    for decl in &program.interfaces { symbols.register_interface(decl); }
    for decl in &program.modules    { symbols.register_module(decl); }
    for decl in &program.enums      { symbols.register_enum(decl); }
    for decl in &program.classes    { symbols.register_class(decl); }
    for decl in &program.functions  { symbols.register_function(decl); }

    // ── 4d. Vérification des interfaces implémentées ──────────────────────────
    for class_decl in &program.classes {
        for iface_name in &class_decl.implements {
            // Vérifier que l'interface existe
            let iface_info = match symbols.lookup_interface(iface_name) {
                Some(info) => info,
                None => {
                    diagnostic::print_error(&args.input, class_decl.span.line, class_decl.span.col,
                        &format!("interface '{}' not found", iface_name));
                    std::process::exit(1);
                }
            };
            
            // Vérifier que la classe implémente toutes les méthodes de l'interface
            for (method_name, _iface_sig) in &iface_info.methods {
                // Chercher la méthode dans la classe (en remontant la chaîne d'héritage)
                let found = symbols.lookup_method_in_chain(&class_decl.name, method_name);
                
                if found.is_none() {
                    diagnostic::print_error(&args.input, class_decl.span.line, class_decl.span.col,
                        &format!("class '{}' does not implement method '{}' from interface '{}'",
                            class_decl.name, method_name, iface_name));
                    std::process::exit(1);
                }
                
                // TODO: vérifier aussi la signature (paramètres et type de retour)
            }
        }
    }

    // ── 4e. Analyse sémantique ────────────────────────────────────────────────
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);

    // Afficher erreurs + warnings triés par ligne (format GCC cliquable)
    let has_errors   = !checker.errors.is_empty();
    let has_warnings = !checker.warnings.is_empty();

    if has_errors || has_warnings {
        // Collecter tous les messages avec leur ligne pour trier
        let mut items: Vec<(usize, usize, bool, String)> = Vec::new();
        for err in &checker.errors {
            items.push((err.span().line, err.span().col, true, err.message()));
        }
        for w in &checker.warnings {
            items.push((w.span().line, w.span().col, false, w.message()));
        }
        items.sort_by_key(|i| (i.0, i.1));

        for (line, col, is_error, msg) in &items {
            if *is_error {
                diagnostic::print_error(&args.input, *line, *col, msg);
            } else {
                diagnostic::print_warn(&args.input, *line, *col, msg);
            }
        }

        if has_errors {
            std::process::exit(1);
        }
    }

    if args.check {
        println!("check ok — no semantic errors.");
        return;
    }

    // ── 5. Lowering AST → Ocara HIR ────────────────────────────────────────────
    let source_file = args.input.to_string_lossy().to_string();
    let ir_module = lower_program(&program, &source_file);

    if args.dump {
        println!("=== HIR ({} fonctions) ===", ir_module.functions.len());
        for func in &ir_module.functions {
            println!("func {} ({} blocs)", func.name, func.blocks.len());
            for bb in &func.blocks {
                println!("  {}:", bb.id);
                for inst in &bb.insts {
                    println!("    {:?}", inst);
                }
            }
        }
        println!();
    }

    // ── 6. Génération de code Cranelift → objet natif ──────────────────────────
    let module_name = args.input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ocara_module");

    let emitter = match CraneliftEmitter::new(module_name) {
        Ok(e) => e,
        Err(e) => {
            diagnostic::print_error(&args.input, 0, 0, &format!("codegen init: {}", e));
            std::process::exit(1);
        }
    };

    let obj_bytes = match emitter.compile(&ir_module) {
        Ok(b) => b,
        Err(e) => {
            diagnostic::print_error(&args.input, 0, 0, &format!("codegen: {}", e));
            std::process::exit(1);
        }
    };

    if args.no_link {
        let obj_path = args.output.with_extension("o");
        if let Err(e) = fs::write(&obj_path, &obj_bytes) {
            diagnostic::print_error(&args.input, 0, 0, &format!("écriture de '{}': {}", obj_path.display(), e));
            std::process::exit(1);
        }
        println!("objet généré: {}", obj_path.display());
        return;
    }

    // ── 8. Liaison finale ─────────────────────────────────────────────────────
    let obj_path = args.output.with_extension("o");
    match link(&obj_bytes, &obj_path, &args.output, args.release) {
        Ok(()) => {
            println!("compilation réussie → {}", args.output.display());
        }
        Err(e) => {
            diagnostic::print_error(&args.input, 0, 0, &format!("link: {}", e));
            std::process::exit(1);
        }
    }
}
