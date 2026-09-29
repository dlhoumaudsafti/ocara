mod builtins;
mod codegen;
mod core;
mod ir;
mod lower;
mod parsing;
mod sema;

use std::fs;

use codegen::emit::CraneliftEmitter;
use codegen::link::{link, link_android};
use lower::builder::lower_program;
use sema::symbols::SymbolTable;
use sema::typecheck::{TypeChecker, type_name, types_compat};

use core::alias_resolve::{compute_aliases, resolve_aliases};
use core::cli::parse_args;
use core::monomorph::monomorphize;
use core::render_file::desugar_render_file;
use core::runtime_expand::{expand_runtime_imports, get_stmt_start_line, get_stmt_end_line, update_program_spans_with_file};
use parsing::{lexer::Lexer, parser::Parser, diagnostic, token};

/// Import implicite de chaque `wiring` d'une interface qui vient d'entrer
/// dans `program.interfaces` (voir les 3 sites d'appel dans la boucle
/// principale — miroir du rapatriement déjà existant "classe importée →
/// ses interfaces implémentées", ici dans l'autre sens : "interface importée
/// → ses classes wired"). Sans ceci, une classe visée par un `wiring` mais
/// jamais importée EXPLICITEMENT par aucun fichier consommateur ne serait
/// jamais chargée dans le programme compilé (voir
/// docs/roadmap.d/langage-interface-wiring.md, "Import implicite").
fn enqueue_wiring_imports(
    iface: &parsing::ast::InterfaceDecl,
    parent_dir: &std::path::Path,
    parent_namespace: &Option<String>,
    imports_to_process: &mut Vec<(parsing::ast::ImportDecl, std::path::PathBuf, Option<String>)>,
    program_classes: &mut Vec<parsing::ast::ClassDecl>,
    local_pool: &[parsing::ast::ClassDecl],
) {
    for w in &iface.wirings {
        let target = w.simple_name();

        // Déjà fusionnée dans le programme : rien à faire.
        if program_classes.iter().any(|c| c.name == target) {
            continue;
        }

        // Une classe `wiring`-ée qui est DÉJÀ déclarée directement dans le
        // fichier qui vient d'être chargé (`local_pool`, typiquement
        // `mod_prog.classes` — l'usage le plus simple de `wiring` colocalise
        // l'interface et sa/ses classe(s) implémentante(s) dans le MÊME
        // fichier, comme les petits exemples illustratifs) n'a besoin
        // d'AUCUN import de fichier séparé : elle est simplement RAPATRIÉE
        // directement (clonée) dans `program.classes` ici — un import de
        // fichier déclenchait sinon TOUJOURS une tentative de charger un
        // fichier séparé du même nom (`PostgresRepo.oc`), qui n'existe pas
        // puisque la classe est juste à côté dans le fichier déjà en main —
        // confirmé par reproduction (`error: reading file
        // '.../PostgresRepo.oc': No such file or directory`). Un doublon
        // inoffensif est possible ici si `local_pool` est ENSUITE fusionné
        // en bloc par l'appelant (import `*`) : nettoyé par le dédoublonnage
        // de §4b, plus loin dans `main()`.
        if let Some(local_cls) = local_pool.iter().find(|c| c.name == target) {
            program_classes.push(local_cls.clone());
            continue;
        }

        let virtual_imp = parsing::ast::ImportDecl {
            path: vec![target.to_string()],
            alias: None,
            file_path: Some(w.path.join("/")),
            span: w.span.clone(),
        };
        imports_to_process.push((virtual_imp, parent_dir.to_path_buf(), parent_namespace.clone()));
    }
}

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
            use parsing::error::LexError;
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

    // Répertoire de base pour la résolution des imports — calculé ICI (avant
    // la résolution des alias du fichier principal, pas après comme avant ce
    // ticket) car le pré-scan `wiring` ci-dessous en a besoin.
    let source_dir = args.src_dir.as_ref()
        .map(|p| p.as_path())
        .unwrap_or_else(|| args.input.parent().unwrap_or_else(|| std::path::Path::new(".")));

    // Pré-scan de TOUTES les interfaces (avec leurs `wiring`) atteignables
    // depuis le fichier principal — voir core::interface_wiring pour la
    // raison (un alias doit connaître les `wiring` d'une interface AVANT que
    // celle-ci ne soit chargée par la boucle principale, qui ne respecte pas
    // un ordre de dépendance). Fait une seule fois, avant toute résolution
    // d'alias — y compris celle du fichier principal juste en dessous.
    let all_interfaces = core::interface_wiring::collect_all_interfaces(&program, source_dir);

    // Résoudre les alias d'import (`import X as Y`) du fichier PRINCIPAL —
    // voir core::alias_resolve pour la raison (renommer le symbole importé,
    // ancien comportement, cassait la résolution partout où un AUTRE
    // fichier référence le même symbole par son vrai nom). Fait ici, avant
    // toute fusion multi-fichiers, sur les SEULS imports de ce fichier —
    // chaque fichier importé reçoit le même traitement plus bas, sur ses
    // propres imports uniquement (un alias n'est jamais visible en dehors
    // du fichier qui l'a écrit).
    let main_file_aliases = compute_aliases(&program.imports, &all_interfaces, &args.input);
    resolve_aliases(&mut program, &main_file_aliases);

    // ── 4. Vérification des imports non-builtins ──────────────────────────────
    // Les modules `ocara.*` sont builtins (livrés avec le runtime).
    // Tout autre import doit pointer vers un fichier .oc existant.
    const OCARA_BUILTINS: &[&str] = &[
        "IO", "Math", "String", "Array", "Map", "JSON", "Tauri", "SDL",
        "Convert", "System", "Regex", "HTTPRequest", "HTTPResponse", "HTTPServer", "HTTPServerRequest", "SQLite", "MySQL", "MariaDB", "DotEnv", "YAML", "Thread", "Mutex",
        "DateTime", "Date", "Time", "UnitTest", "HTMLComponent", "HTML",
        "File", "Directory", "Exception", "FileException", "DirectoryException", "IOException", "SystemException",
        "ArrayException", "MapException", "MathException", "ConvertException", "RegexException",
        "DateTimeException", "DateException", "TimeException",
        "ThreadException", "MutexException",
        "UnitTestException", "HTTPServerException", "SQLiteException", "MySQLException", "MariaDBException", "DotEnvException", "YAMLException",
        "SDLException", "TauriException",
    ];

    // Séparer les imports en deux catégories
    let module_imports: Vec<parsing::ast::ImportDecl> = program.imports.iter()
        .filter(|imp| imp.file_path.is_none() && imp.path.first().map(|s| s.as_str()) != Some("ocara"))
        .cloned()
        .collect();
    
    let file_imports: Vec<parsing::ast::ImportDecl> = program.imports.iter()
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
    // Déduplication par (fichier, symbole demandé) et non par fichier seul :
    // `import Circle from "Geometry"` puis `import Rectangle from "Geometry"`
    // (cas d'usage documenté EBNF §4.3 "fichier multi-classes") doivent tous
    // les deux être traités, même si le fichier a déjà été chargé pour un
    // autre symbole. Le programme parsé de chaque fichier est mis en cache
    // pour éviter de le relire/reparser à chaque symbole demandé.
    let mut processed_imports: std::collections::HashSet<(std::path::PathBuf, String)> = std::collections::HashSet::new();
    let mut parsed_files_cache: std::collections::HashMap<std::path::PathBuf, parsing::ast::Program> = std::collections::HashMap::new();

    // (ImportDecl, répertoire du fichier parent, namespace du fichier parent)
    let mut imports_to_process: Vec<(parsing::ast::ImportDecl, std::path::PathBuf, Option<String>)> = Vec::new();
    
    // Namespace du fichier principal
    let main_namespace = program.namespace.clone();
    
    // Ajouter les imports "from" (nouveau format)
    for imp in &file_imports {
        imports_to_process.push((imp.clone(), source_dir.to_path_buf(), main_namespace.clone()));
    }
    
    // Ajouter les imports namespace (ancien format) - créer un ImportDecl virtuel avec file_path
    for imp in &module_imports {
        let file_path_str = imp.path.join("/");
        
        // Le dernier segment est le nom du symbole à importer
        let symbol_name = imp.path.last().cloned().unwrap_or_default();
        
        // Créer un import virtuel avec file_path pour le traiter comme un import "from"
        let virtual_imp = parsing::ast::ImportDecl {
            path: vec![symbol_name], // importer ce symbole spécifique
            alias: imp.alias.clone(),
            file_path: Some(file_path_str),
            span: imp.span.clone(),
        };
        imports_to_process.push((virtual_imp, source_dir.to_path_buf(), main_namespace.clone()));
    }

    // Import implicite des `wiring` d'une interface déclarée DIRECTEMENT
    // dans le fichier principal (rare — le cas courant est une interface
    // importée depuis un autre fichier, déjà couvert aux 3 sites d'appel de
    // `enqueue_wiring_imports` dans la boucle ci-dessous, mais rien
    // n'empêche `wiring` d'apparaître dans le fichier passé à `ocara build`
    // lui-même).
    for iface in &program.interfaces {
        enqueue_wiring_imports(iface, source_dir, &main_namespace, &mut imports_to_process, &mut program.classes, &[]);
    }

    while !imports_to_process.is_empty() {
        let (imp, parent_dir, parent_namespace) = imports_to_process.remove(0);
        let file_path_str = imp.file_path.as_ref().unwrap();
        
        // Résoudre le chemin depuis le répertoire parent
        let clean_path = file_path_str.trim_end_matches(".oc");
        let mut file_path;
        
        if clean_path.starts_with("../") || clean_path.starts_with("./") {
            // Chemin relatif : résoudre depuis le répertoire parent
            file_path = parent_dir.join(clean_path);
            if !file_path.extension().is_some() {
                file_path.set_extension("oc");
            }
        } else {
            // Chemin depuis namespace courant ou racine
            // 1. D'abord essayer dans le namespace courant (si on en a un)
            if parent_namespace.is_some() && parent_namespace.as_deref() != Some(".") {
                let ns = parent_namespace.as_ref().unwrap().replace(".", "/");
                file_path = source_dir.join(&ns).join(clean_path).with_extension("oc");
                
                // Si trouvé dans le namespace, on s'arrête
                if !file_path.exists() {
                    // 2. Sinon essayer à la racine
                    file_path = source_dir.join(clean_path).with_extension("oc");
                }
            } else {
                // Pas de namespace, chercher directement à la racine
                file_path = source_dir.join(clean_path).with_extension("oc");
            }
        }
        
        // Éviter de traiter deux fois le même symbole depuis le même fichier
        // (mais pas le fichier entier : deux imports distincts d'un même
        // fichier multi-classes, ex. `import Circle from "Geometry"` puis
        // `import Rectangle from "Geometry"`, doivent chacun être traités).
        let canonical_path = file_path.canonicalize().unwrap_or(file_path.clone());
        let requested_key = if imp.path.first().map(|s| s == "*").unwrap_or(false) {
            "*".to_string()
        } else {
            imp.path.first().cloned().unwrap_or_default()
        };
        if processed_imports.contains(&(canonical_path.clone(), requested_key.clone())) {
            continue;
        }
        processed_imports.insert((canonical_path.clone(), requested_key));

        // Le répertoire parent pour les imports de ce fichier
        let current_file_dir = file_path.parent().unwrap_or(&parent_dir).to_path_buf();

        // Réutilise le programme déjà parsé si un import précédent a déjà
        // chargé ce même fichier pour un autre symbole.
        let mut mod_prog = if let Some(cached) = parsed_files_cache.get(&canonical_path) {
            cached.clone()
        } else {
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
            let parsed = match Parser::new(mod_tokens).parse_program() {
                Ok(p) => p,
                Err(e) => {
                    diagnostic::print_error(&file_path, e.span.line, e.span.col, &e.message);
                    std::process::exit(1);
                }
            };
            parsed_files_cache.insert(canonical_path.clone(), parsed.clone());
            parsed
        };

        // Mettre à jour tous les spans du programme importé avec le nom du fichier
        update_program_spans_with_file(&mut mod_prog, &file_path.to_string_lossy());

        // Namespace de CE fichier — remonté ici (avant, calculé seulement
        // après la fusion plus bas) : nécessaire dès maintenant pour
        // rapatrier correctement les `wiring` d'une interface qui viendrait
        // d'être fusionnée depuis ce fichier (voir `enqueue_wiring_imports`
        // ci-dessous, 3 sites d'appel).
        let loaded_namespace = mod_prog.namespace.clone();

        // Résoudre les alias d'import (`import X as Y`) écrits DANS ce
        // fichier lui-même, sur ses propres imports uniquement — voir
        // core::alias_resolve et le même appel plus haut pour le fichier
        // principal. Fait avant toute extraction/fusion : les symboles de
        // `mod_prog` ne portent plus jamais un alias au moment d'être
        // copiés dans `program`.
        let mod_file_aliases = compute_aliases(&mod_prog.imports, &all_interfaces, &file_path);
        resolve_aliases(&mut mod_prog, &mod_file_aliases);

        // Extraire ce qui est demandé
        let is_wildcard = imp.path.first().map(|s| s == "*").unwrap_or(false);

        if is_wildcard {
            // import * from "file" → tout importer
            for iface in &mod_prog.interfaces {
                // `local_pool = &mod_prog.classes` : nécessaire même si
                // `import *` fusionne de toute façon TOUT `mod_prog.classes`
                // trois lignes plus bas — SANS CE `local_pool` ICI, une classe
                // `wiring`-ée colocalisée dans ce même fichier ne serait pas
                // encore visible dans `program.classes` à CET instant (avant
                // le `.extend` ci-dessous) et déclencherait à tort une
                // tentative de fichier séparé. Le doublon temporaire que ce
                // choix peut produire (classe rapatriée ICI puis refusionnée
                // par le `.extend`) est inoffensif : nettoyé par le
                // dédoublonnage §4b plus loin dans `main()`.
                enqueue_wiring_imports(iface, &current_file_dir, &loaded_namespace, &mut imports_to_process, &mut program.classes, &mod_prog.classes);
            }
            program.classes.extend(mod_prog.classes);
            program.interfaces.extend(mod_prog.interfaces);
            program.functions.extend(mod_prog.functions);
            program.consts.extend(mod_prog.consts);
            program.modules.extend(mod_prog.modules);
            program.generics.extend(mod_prog.generics);
        } else {
            // import Circle from "file" → importer seulement Circle
            let requested_name = imp.path.first().cloned().unwrap_or_default();

            // Rapatrier aussi les constantes de fichier (même logique que
            // `import *` ci-dessus) : une classe/fonction importée peut
            // référencer une const de premier niveau de son fichier d'origine
            // (ex: `Score::is_passing()` lisant `PASS_MARK`, jamais liée à un
            // symbole précis contrairement à un `implements` — voir le
            // rapatriement des interfaces ci-dessous). Confirmé par
            // reproduction (`examples/project/tests/mainTest.oc`) : sans ça,
            // un import sélectif casse dès que le symbole importé dépend
            // d'une const de son fichier — voir
            // docs/roadmap.d/langage-imports-modules.md.
            for c in &mod_prog.consts {
                if !program.consts.iter().any(|existing| existing.name == c.name) {
                    program.consts.push(c.clone());
                }
            }

            // Ordre de priorité: class → generic → interface → module → function
            
            // Chercher la classe
            //
            // Le symbole GARDE son vrai nom (`requested_name`) même si cet
            // import est aliasé (`import X as Y`) — voir core::alias_resolve :
            // le renommer ici casserait la résolution partout où un AUTRE
            // fichier référence le même symbole par son vrai nom (typage de
            // paramètre, `extends`...), puisqu'il n'existerait alors plus
            // sous ce nom nulle part dans le programme fusionné. L'alias
            // lui-même a déjà été réécrit vers le vrai nom, PARTOUT où le
            // fichier qui l'a écrit l'utilise, par l'appel à `resolve_aliases`
            // ci-dessus (fichier principal) / plus haut (fichier importé) —
            // aucun autre traitement n'est donc nécessaire ici.
            if let Some(cls) = mod_prog.classes.iter().find(|c| c.name == requested_name).cloned() {
                // Rapatrier les interfaces implémentées par cette classe, même si
                // elles n'ont pas été explicitement demandées par l'import : sinon
                // la vérification E09 échoue plus loin avec "interface not found"
                // pour une interface pourtant définie dans le même fichier source.
                for iface_name in &cls.implements {
                    if !program.interfaces.iter().any(|i| &i.name == iface_name) {
                        if let Some(iface) = mod_prog.interfaces.iter().find(|i| &i.name == iface_name).cloned() {
                            enqueue_wiring_imports(&iface, &current_file_dir, &loaded_namespace, &mut imports_to_process, &mut program.classes, &mod_prog.classes);
                            program.interfaces.push(iface);
                        }
                    }
                }
                program.classes.push(cls);
            }
            // Chercher le générique
            else if let Some(generic_item) = mod_prog.generics.iter().find(|g| g.name == requested_name).cloned() {
                program.generics.push(generic_item);
            }
            // Chercher l'interface
            else if let Some(iface) = mod_prog.interfaces.iter().find(|i| i.name == requested_name).cloned() {
                enqueue_wiring_imports(&iface, &current_file_dir, &loaded_namespace, &mut imports_to_process, &mut program.classes, &mod_prog.classes);
                program.interfaces.push(iface);
            }
            // Chercher le module
            else if let Some(module) = mod_prog.modules.iter().find(|m| m.name == requested_name).cloned() {
                program.modules.push(module);
            }
            // Chercher la fonction
            else if let Some(func) = mod_prog.functions.iter().find(|f| f.name == requested_name).cloned() {
                program.functions.push(func);
            }
            else {
                diagnostic::print_error(&args.input, imp.span.line, imp.span.col,
                    &format!("'{}' not found in file '{}'", requested_name, file_path_str));
                std::process::exit(1);
            }
        }
        
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
                let file_path_str = new_imp.path.join("/");
                let symbol_name = new_imp.path.last().cloned().unwrap_or_default();
                
                let virtual_imp = parsing::ast::ImportDecl {
                    path: vec![symbol_name],
                    alias: new_imp.alias.clone(),
                    file_path: Some(file_path_str),
                    span: new_imp.span.clone(),
                };
                imports_to_process.push((virtual_imp, source_dir.to_path_buf(), loaded_namespace.clone()));
            }
        }
    }

    // NOTE : `module_imports` (ancien format `import module.Path`) est déjà
    // entièrement traité ci-dessus — chaque entrée est convertie en import
    // virtuel "from" (ligne ~178) et chargée par le chemin récursif unique
    // au-dessus, qui gère classes/generics/interfaces/modules/functions,
    // l'alias et la résolution par namespace. Un second chemin, redondant et
    // incomplet (relecture du même fichier, fusion inconditionnelle de
    // TOUTES les classes/functions/consts sans regarder ce qui est demandé,
    // interfaces et modules jamais fusionnés), vivait ici — supprimé : il
    // masquait silencieusement l'absence d'interfaces transitives dès que le
    // chemin principal résolvait le symbole demandé comme autre chose qu'une
    // classe (confirmé par reproduction sur `examples/project/tests/mainTest.oc`,
    // qui importe la FONCTION `main` d'un fichier définissant aussi des
    // classes `implements Printable`/`Comparable` — voir
    // docs/roadmap.d/langage-imports-modules.md).

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

    // ── 4b-bis. `wiring` : substitution des noms nus (SANS alias) en position
    // construction/appel statique vers le PREMIER wiring déclaré — voir
    // core::interface_wiring et docs/roadmap.d/langage-interface-wiring.md.
    // Tourne APRÈS la fusion complète (tous les `wiring` connus, aucun souci
    // d'ordre à ce stade, contrairement à la résolution d'alias) et AVANT la
    // construction de la table des symboles/le typecheck, pour qu'ils voient
    // directement la classe concrète wired, jamais l'interface elle-même.
    core::interface_wiring::resolve_bare_interface_names(&mut program, &all_interfaces);

    // ── 4c. Construction de la table des symboles ─────────────────────────────
    let mut symbols = SymbolTable::new();
    for decl in &program.imports    { symbols.register_import(decl); }
    for decl in &program.consts     { symbols.register_const(decl); }
    for decl in &program.interfaces { symbols.register_interface(decl); }
    for decl in &program.modules    { symbols.register_module(decl); }
    for decl in &program.enums      { symbols.register_enum(decl); }
    for decl in &program.classes    { symbols.register_class(decl); }
    for decl in &program.generics   { symbols.register_generic(decl); }
    for decl in &program.functions  { symbols.register_function(decl); }

    // ── 4c-bis. Vérification de l'existence du parent `extends` (E27) ────────
    // Ni une classe ni un `generic` ne vérifiaient que leur `extends`
    // désignait bien une classe/generic connue — un parent inexistant
    // compilait silencieusement (confirmé par reproduction : `class Foo
    // extends DoesNotExist { }` compile et s'exécute sans la moindre
    // erreur). Un `generic` peut désigner soit une classe concrète, soit un
    // autre `generic` comme parent (voir `extends_args`, arguments de type
    // pour `extends Base<T>`) — les deux tables sont donc consultées.
    for class_decl in &program.classes {
        if let Some(parent) = &class_decl.extends {
            if symbols.lookup_class(parent).is_none() {
                diagnostic::print_error(&args.input, class_decl.span.line, class_decl.span.col,
                    &format!("class '{}' extends unknown class '{}'", class_decl.name, parent));
                std::process::exit(1);
            }
        }
    }
    for generic_decl in &program.generics {
        if let Some(parent) = &generic_decl.extends {
            if symbols.lookup_class(parent).is_none() && symbols.lookup_generic(parent).is_none() {
                diagnostic::print_error(&args.input, generic_decl.span.line, generic_decl.span.col,
                    &format!("generic '{}' extends unknown class/generic '{}'", generic_decl.name, parent));
                std::process::exit(1);
            }
        }
    }

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
            for (method_name, iface_sig) in &iface_info.methods {
                // Chercher la méthode dans la classe (en remontant la chaîne d'héritage)
                let class_sig = match symbols.lookup_method_in_chain(&class_decl.name, method_name) {
                    Some(sig) => sig,
                    None => {
                        diagnostic::print_error(&args.input, class_decl.span.line, class_decl.span.col,
                            &format!("class '{}' does not implement method '{}' from interface '{}'",
                                class_decl.name, method_name, iface_name));
                        std::process::exit(1);
                    }
                };

                // Vérifier la signature : staticité, arité, types des paramètres, type de retour
                //
                // `is_static` — nouveau depuis `wiring` (une interface peut
                // désormais déclarer un contrat STATIQUE, voir
                // `parse_interface_method`/`InterfaceMethod::is_static`,
                // docs/roadmap.d/langage-interface-wiring.md) : cette
                // vérification s'applique à TOUT `implements`, pas seulement
                // aux classes visées par un `wiring` — une interface qui
                // exige une méthode statique doit être honorée par une
                // méthode statique, jamais d'instance, et réciproquement.
                if class_sig.is_static != iface_sig.is_static {
                    diagnostic::print_error(&args.input, class_decl.span.line, class_decl.span.col,
                        &format!("method '{}' of class '{}' does not match interface '{}': expected a {} method, found a {} method",
                            method_name, class_decl.name, iface_name,
                            if iface_sig.is_static { "static" } else { "instance" },
                            if class_sig.is_static { "static" } else { "instance" }));
                    std::process::exit(1);
                }
                // `is_async` — même famille que `is_static` ci-dessus, voir
                // docs/roadmap.d/langage-interface-method-modifiers.md :
                // sémantique directe et sans ambiguïté (contrairement à la
                // visibilité, volontairement jamais vérifiée nulle part dans
                // ce compilateur, voir la doc de `InterfaceMethod`) — une
                // interface qui exige `async method` doit être honorée par
                // une méthode `async`, et réciproquement.
                if class_sig.is_async != iface_sig.is_async {
                    diagnostic::print_error(&args.input, class_decl.span.line, class_decl.span.col,
                        &format!("method '{}' of class '{}' does not match interface '{}': expected an '{}' method, found an '{}' method",
                            method_name, class_decl.name, iface_name,
                            if iface_sig.is_async { "async" } else { "non-async" },
                            if class_sig.is_async { "async" } else { "non-async" }));
                    std::process::exit(1);
                }
                if class_sig.params.len() != iface_sig.params.len() {
                    diagnostic::print_error(&args.input, class_decl.span.line, class_decl.span.col,
                        &format!("method '{}' of class '{}' does not match interface '{}': expected {} parameter(s), found {}",
                            method_name, class_decl.name, iface_name, iface_sig.params.len(), class_sig.params.len()));
                    std::process::exit(1);
                }
                for (i, (_, iface_param_ty)) in iface_sig.params.iter().enumerate() {
                    let (_, class_param_ty) = &class_sig.params[i];
                    if !types_compat(class_param_ty, iface_param_ty, &symbols) {
                        diagnostic::print_error(&args.input, class_decl.span.line, class_decl.span.col,
                            &format!("method '{}' of class '{}' does not match interface '{}': parameter {} expected type '{}', found '{}'",
                                method_name, class_decl.name, iface_name, i + 1,
                                type_name(iface_param_ty), type_name(class_param_ty)));
                        std::process::exit(1);
                    }
                }
                if !types_compat(&class_sig.ret_ty, &iface_sig.ret_ty, &symbols) {
                    diagnostic::print_error(&args.input, class_decl.span.line, class_decl.span.col,
                        &format!("method '{}' of class '{}' does not match interface '{}': expected return type '{}', found '{}'",
                            method_name, class_decl.name, iface_name,
                            type_name(&iface_sig.ret_ty), type_name(&class_sig.ret_ty)));
                    std::process::exit(1);
                }
            }
        }
    }

    // ── 4d-bis. Vérification des interfaces implémentées par un `generic` ────
    // Même vérification que ci-dessus, mais pour `program.generics` : la
    // monomorphisation (plus bas, `monomorphize(&mut program)`) transforme
    // chaque instanciation en classe concrète dans `program.classes`, mais
    // seulement APRÈS ce point — la boucle ci-dessus ne voit donc jamais un
    // `generic` (même avec un `implements` invalide/incomplet), quel que soit
    // le nombre de fois où il est instancié. Confirmé par reproduction : voir
    // docs/roadmap.d/langage-generiques.md.
    for generic_decl in &program.generics {
        for iface_name in &generic_decl.implements {
            let iface_info = match symbols.lookup_interface(iface_name) {
                Some(info) => info,
                None => {
                    diagnostic::print_error(&args.input, generic_decl.span.line, generic_decl.span.col,
                        &format!("interface '{}' not found", iface_name));
                    std::process::exit(1);
                }
            };

            let generic_info = symbols.lookup_generic(&generic_decl.name)
                .expect("le generic vient d'être enregistré ci-dessus (4c)");

            for (method_name, iface_sig) in &iface_info.methods {
                let class_sig = match generic_info.methods.get(method_name) {
                    Some(sig) => sig,
                    None => {
                        diagnostic::print_error(&args.input, generic_decl.span.line, generic_decl.span.col,
                            &format!("generic '{}' does not implement method '{}' from interface '{}'",
                                generic_decl.name, method_name, iface_name));
                        std::process::exit(1);
                    }
                };

                // Staticité — même remarque que pour la boucle 4d ci-dessus.
                if class_sig.is_static != iface_sig.is_static {
                    diagnostic::print_error(&args.input, generic_decl.span.line, generic_decl.span.col,
                        &format!("method '{}' of generic '{}' does not match interface '{}': expected a {} method, found a {} method",
                            method_name, generic_decl.name, iface_name,
                            if iface_sig.is_static { "static" } else { "instance" },
                            if class_sig.is_static { "static" } else { "instance" }));
                    std::process::exit(1);
                }
                // `is_async` — même remarque que pour la boucle 4d ci-dessus.
                if class_sig.is_async != iface_sig.is_async {
                    diagnostic::print_error(&args.input, generic_decl.span.line, generic_decl.span.col,
                        &format!("method '{}' of generic '{}' does not match interface '{}': expected an '{}' method, found an '{}' method",
                            method_name, generic_decl.name, iface_name,
                            if iface_sig.is_async { "async" } else { "non-async" },
                            if class_sig.is_async { "async" } else { "non-async" }));
                    std::process::exit(1);
                }
                if class_sig.params.len() != iface_sig.params.len() {
                    diagnostic::print_error(&args.input, generic_decl.span.line, generic_decl.span.col,
                        &format!("method '{}' of generic '{}' does not match interface '{}': expected {} parameter(s), found {}",
                            method_name, generic_decl.name, iface_name, iface_sig.params.len(), class_sig.params.len()));
                    std::process::exit(1);
                }
                for (i, (_, iface_param_ty)) in iface_sig.params.iter().enumerate() {
                    let (_, class_param_ty) = &class_sig.params[i];
                    if !types_compat(class_param_ty, iface_param_ty, &symbols) {
                        diagnostic::print_error(&args.input, generic_decl.span.line, generic_decl.span.col,
                            &format!("method '{}' of generic '{}' does not match interface '{}': parameter {} expected type '{}', found '{}'",
                                method_name, generic_decl.name, iface_name, i + 1,
                                type_name(iface_param_ty), type_name(class_param_ty)));
                        std::process::exit(1);
                    }
                }
                if !types_compat(&class_sig.ret_ty, &iface_sig.ret_ty, &symbols) {
                    diagnostic::print_error(&args.input, generic_decl.span.line, generic_decl.span.col,
                        &format!("method '{}' of generic '{}' does not match interface '{}': expected return type '{}', found '{}'",
                            method_name, generic_decl.name, iface_name,
                            type_name(&iface_sig.ret_ty), type_name(&class_sig.ret_ty)));
                    std::process::exit(1);
                }
            }
        }
    }

    // ── 4d-ter. Vérification des `wiring` déclarés par les interfaces ────────
    // Voir docs/roadmap.d/langage-interface-wiring.md. La compatibilité de
    // SIGNATURE (arité, staticité, types de paramètres/retour) entre une
    // classe wired et l'interface est déjà entièrement couverte par la
    // boucle 4d ci-dessus dès lors que `implements` mentionne bien cette
    // interface (E09) — il ne reste donc à vérifier ICI que ce que 4d ne
    // couvre pas : l'existence de la classe cible, le fait qu'elle déclare
    // bien `implements` cette interface (sinon 4d ne la voit jamais), et
    // l'absence de doublon de nom simple entre les `wiring` d'une même
    // interface (ce qui rendrait la résolution "premier gagnant" ambiguë
    // pour la substitution d'alias, voir core::alias_resolve).
    for iface_decl in &program.interfaces {
        // E42 — deux `wiring` de la même interface partageant le même nom
        // simple (dernier segment du chemin pointé) : la résolution par
        // alias (core::alias_resolve::compute_aliases) ne pourrait plus
        // savoir laquelle des deux cibles un alias `import Interface as X`
        // désigne réellement. Signalé sur la déclaration de l'INTERFACE
        // elle-même (et non sur une classe), en pointant les deux `wiring`
        // en cause.
        for i in 0..iface_decl.wirings.len() {
            for j in (i + 1)..iface_decl.wirings.len() {
                if iface_decl.wirings[i].simple_name() == iface_decl.wirings[j].simple_name() {
                    diagnostic::print_error(&args.input, iface_decl.span.line, iface_decl.span.col,
                        &format!("interface '{}' declares two 'wiring' targets with the same simple name '{}' ({}:{} and {}:{}) — alias resolution could not tell them apart",
                            iface_decl.name, iface_decl.wirings[i].simple_name(),
                            iface_decl.wirings[i].span.line, iface_decl.wirings[i].span.col,
                            iface_decl.wirings[j].span.line, iface_decl.wirings[j].span.col));
                    std::process::exit(1);
                }
            }
        }

        for wiring in &iface_decl.wirings {
            let target_name = wiring.simple_name();

            // E39 — classe cible introuvable (ni classe, ni generic connu).
            let target_class = match symbols.lookup_class(target_name) {
                Some(info) => info,
                None => {
                    if symbols.lookup_generic(target_name).is_some() {
                        // Un `generic` n'est monomorphisé en classe concrète
                        // qu'après ce point (voir remarque 4d-bis ci-dessus) —
                        // `wiring` vers un `generic` nu n'a pas de sens (quelle
                        // instanciation choisir ?), donc explicitement rejeté,
                        // avec un message dédié plutôt que "introuvable".
                        diagnostic::print_error(&args.input, wiring.span.line, wiring.span.col,
                            &format!("interface '{}': 'wiring {}' targets a generic, not a concrete class — wiring a bare generic is ambiguous (which instantiation?)",
                                iface_decl.name, target_name));
                        std::process::exit(1);
                    }
                    diagnostic::print_error(&args.input, wiring.span.line, wiring.span.col,
                        &format!("interface '{}': 'wiring {}' target class not found", iface_decl.name, target_name));
                    std::process::exit(1);
                }
            };

            // E40 — la classe cible n'`implements` pas (textuellement) cette
            // interface. Sans cette vérification, la boucle 4d ne verrait
            // JAMAIS cette classe (elle n'itère que `class_decl.implements`),
            // et une classe wired incompatible ne serait donc jamais
            // signalée avant de produire un mauvais résultat silencieux à
            // l'exécution (méthode manquante → `Type::Mixed` permissif en
            // amont, mais aucun symbole réel côté codegen).
            if !target_class.implements.iter().any(|i| i == &iface_decl.name) {
                diagnostic::print_error(&args.input, wiring.span.line, wiring.span.col,
                    &format!("interface '{}': 'wiring {}' target class '{}' does not 'implements {}'",
                        iface_decl.name, target_name, target_name, iface_decl.name));
                std::process::exit(1);
            }
        }
    }

    // ── 4d. Expansion des imports runtime ─────────────────────────────────────
    expand_runtime_imports(&mut program, source_dir, &args.input);

    // ── 4d-bis. Désucrage HTML::renderFile / HTML::renderFileCached ───────────
    // Doit tourner avant le typecheck : le fichier est lu à la compilation et
    // réécrit en HTML::render(...)/HTML::renderCached(...) avec un vrai
    // template, pour que l'analyse sémantique (dont la détection des
    // variables "unused") voie les mêmes expressions qu'un littéral backtick.
    if let Err((span, msg)) = desugar_render_file(&mut program) {
        diagnostic::print_error(&args.input, span.line, span.col, &msg);
        std::process::exit(1);
    }

    // ── 4e. Analyse sémantique ────────────────────────────────────────────────
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);

    // Afficher erreurs + warnings triés par ligne (format GCC cliquable)
    let has_errors   = !checker.errors.is_empty();
    let has_warnings = !checker.warnings.is_empty();

    if has_errors || has_warnings {
        // Créer une map des plages de lignes pour chaque bloc runtime
        let mut runtime_ranges: Vec<(std::ops::Range<usize>, &str)> = Vec::new();
        for block in &program.runtime_blocks {
            if let (Some(first), Some(last)) = (block.statements.first(), block.statements.last()) {
                // Extraire le span du premier et dernier statement
                let start_line = get_stmt_start_line(first);
                let end_line = get_stmt_end_line(last);
                if start_line > 0 && end_line > 0 {
                    runtime_ranges.push((start_line..end_line + 1, block.kind.as_str()));
                }
            }
        }
        
        // Collecter tous les messages avec leur ligne pour trier
        let mut items: Vec<(usize, usize, bool, String, Option<String>, Option<String>)> = Vec::new();
        for err in &checker.errors {
            items.push((err.span().line, err.span().col, true, err.message(), err.span().file.clone(), err.span().runtime_ctx.clone()));
        }
        for w in &checker.warnings {
            items.push((w.span().line, w.span().col, false, w.message(), w.span().file.clone(), w.span().runtime_ctx.clone()));
        }
        items.sort_by_key(|i| (i.0, i.1));

        for (line, col, is_error, msg, file_opt, runtime_ctx_opt) in &items {
            // Utiliser le fichier du span si disponible, sinon args.input
            let file_path = file_opt.as_ref()
                .map(|f| std::path::PathBuf::from(f))
                .unwrap_or_else(|| args.input.clone());
            
            // Utiliser le contexte runtime du span s'il existe, sinon chercher dans runtime_ranges
            let runtime_ctx = if runtime_ctx_opt.is_some() {
                runtime_ctx_opt.as_deref()
            } else {
                // Fallback : chercher dans runtime_ranges (pour les erreurs sans contexte)
                runtime_ranges.iter()
                    .find(|(range, _)| range.contains(line))
                    .map(|(_, kind)| *kind)
            };
            
            if *is_error {
                diagnostic::print_error_ctx(&file_path, *line, *col, msg, runtime_ctx);
            } else {
                diagnostic::print_warn_ctx(&file_path, *line, *col, msg, runtime_ctx);
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

    // ── 4f. Monomorphisation des génériques ───────────────────────────────────
    monomorphize(&mut program);

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

    // La liaison finale pour une cible croisée n'est supportée QUE pour
    // Android, et seulement quand un runtime pré-compilé pour cette cible est
    // fourni (--android-runtime, sous-chantiers 2/3 de packaging-android.md —
    // ce runtime n'est PAS embarqué dans `ocara` comme l'est celui de l'hôte,
    // voir sa doc dans core/cli.rs). Dans tout autre cas (autre cible
    // croisée, ou Android sans runtime fourni), produire un binaire cassé
    // serait pire que refuser : `--target` exige alors `--no-link`.
    let android_link_requested = args.target.as_deref().is_some_and(|t| t.contains("android"))
        && args.android_runtime.is_some();
    if args.target.is_some() && !args.no_link && !android_link_requested {
        diagnostic::print_error(&args.input, 0, 0,
            "--target exige --no-link, sauf pour une cible Android avec --android-runtime fourni (voir docs/roadmap.d/packaging-android.md)");
        std::process::exit(1);
    }

    let emitter = match CraneliftEmitter::new(module_name, args.target.as_deref()) {
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
    // Ne lier libocara_runtime_tauri.a + GTK/WebKit que si le programme importe
    // réellement ocara.Tauri (voir la doc dans src/codegen/link.rs).
    let needs_tauri = ir_module.imports.iter().any(|m| m == "Tauri");
    let needs_sdl = ir_module.imports.iter().any(|m| m == "SDL");

    if android_link_requested {
        // Tauri n'a structurellement aucun équivalent Android (GTK ne tourne
        // pas sur Android) — mais un programme peut légitimement importer
        // ocara.Tauri pour sa seule branche desktop (`if System::OS equal
        // "android" { ... } else { use Tauri(...) }`, le patron établi par
        // examples/advanced/mini_project) : rejeter catégoriquement la
        // compilation dans ce cas empêchait un point d'entrée unique
        // multi-plateforme. `CraneliftEmitter::predeclare_functions` (voir
        // `is_android_target`) compile désormais chaque `Tauri_*` en talon
        // local no-op sur cette cible — le `.o` produit ne référence donc
        // JAMAIS `libocara_runtime_tauri.a`/GTK, juste un avertissement pour
        // que ça reste visible (un appel Tauri atteint par erreur sur Android
        // ne ferait rien, silencieusement, plutôt que de planter).
        // SDL, lui, est supporté depuis la vérification du sous-chantier 4
        // (packaging-android.md) — mais seulement si le runtime SDL Android
        // correspondant est fourni ; sans lui, produire un `.so` qui référence
        // des symboles SDL non résolus serait la même famille de bug que
        // cette session a passé son temps à corriger côté codegen (voir
        // docs/roadmap.d/langage-use-chaine-valeur-retour-perdue.md).
        if needs_tauri {
            diagnostic::print_warn(&args.input, 0, 0,
                "ocara.Tauri est importé mais compilé en talon no-op sur Android (GTK n'a aucun équivalent Android) — tout appel Tauri effectivement atteint au runtime sur cette cible ne fera rien, voir docs/roadmap.d/packaging-android.md");
        }
        if needs_sdl && args.android_runtime_sdl.is_none() {
            diagnostic::print_error(&args.input, 0, 0,
                "ocara.SDL sur Android requiert --android-runtime-sdl (voir `make build-runtime-sdl-android`, docs/roadmap.d/packaging-android.md)");
            std::process::exit(1);
        }

        let target = args.target.as_deref().unwrap();
        let runtime_lib = args.android_runtime.as_ref().unwrap();
        let runtime_sdl_lib = if needs_sdl { args.android_runtime_sdl.as_deref() } else { None };
        let ndk_home = match args.android_ndk.clone()
            .or_else(|| std::env::var_os("ANDROID_NDK_HOME").map(std::path::PathBuf::from))
        {
            Some(p) => p,
            None => {
                diagnostic::print_error(&args.input, 0, 0,
                    "--android-ndk ou $ANDROID_NDK_HOME requis pour lier une cible Android");
                std::process::exit(1);
            }
        };

        match link_android(&obj_bytes, &obj_path, &args.output, target, &ndk_home, runtime_lib, runtime_sdl_lib, args.android_jni_bridge.as_deref(), args.release) {
            Ok(()) => {
                println!("compilation réussie (Android {}) → {}", target, args.output.display());
            }
            Err(e) => {
                diagnostic::print_error(&args.input, 0, 0, &format!("link (android): {}", e));
                std::process::exit(1);
            }
        }
        return;
    }

    match link(&obj_bytes, &obj_path, &args.output, args.release, needs_tauri, needs_sdl) {
        Ok(()) => {
            println!("compilation réussie → {}", args.output.display());
        }
        Err(e) => {
            diagnostic::print_error(&args.input, 0, 0, &format!("link: {}", e));
            std::process::exit(1);
        }
    }
}
