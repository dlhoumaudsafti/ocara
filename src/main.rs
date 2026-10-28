mod builtins;
mod codegen;
mod core;
mod ir;
mod lower;
mod lsp;
mod parsing;
mod sema;

use std::fs;

use codegen::emit::CraneliftEmitter;
use codegen::link::{android_feature_libs, link, link_android};
use lower::builder::lower_program_with;

use core::analysis::{analyze, AnalyzeOptions};
use core::cli::parse_args;
use core::monomorph::monomorphize;
use parsing::diagnostic;

/// Pile du thread de compilation : parseur descendant et passes récursives
/// sur l'arbre (une expression de 800 niveaux débordait les 8 Mio du thread
/// principal). Réservée virtuellement, seules les pages touchées sont
/// allouées.
const COMPILER_STACK_SIZE: usize = 1 << 30;

fn main() {
    let compiler = std::thread::Builder::new()
        .name("ocara".into())
        .stack_size(COMPILER_STACK_SIZE)
        .spawn(run)
        .expect("thread de compilation");
    if compiler.join().is_err() {
        std::process::exit(101);
    }
}

fn run() {
    let args = parse_args();
    if args.lsp {
        lsp::run();
        return;
    }

    if args.input.to_string_lossy().ends_with(".oc.ui") {
        check_ui_file(&args);
        return;
    }

    if args.architecture_report {
        let root = args.src_dir.clone().unwrap_or_else(|| args.input.parent().map(|p| p.to_path_buf()).unwrap_or_default());
        println!("{}", core::analysis::architecture_report::report(&args.input, &root));
        return;
    }

    // Constantes de build des clauses `when` (et de `System::BUILD`).
    let mut config = parsing::build::BuildConfig::for_target(args.target.as_deref(), args.release);
    config.values.extend(args.when.iter().cloned());
    parsing::build::set_current(config);

    // ── 1-4. Analyse : lecture, parsing, imports, vérifications, sema ─────────
    let analysis = analyze(&AnalyzeOptions {
        input: &args.input,
        src_dir: args.src_dir.as_deref(),
        dump: args.dump,
        index: false,
        tolerant: false,
        architecture_only: args.check_architecture,
    });
    for d in &analysis.diagnostics {
        d.print();
    }
    if analysis.has_errors() {
        std::process::exit(1);
    }
    if args.check_architecture {
        println!("architecture ok");
        return;
    }
    let mut program = analysis.checked.expect("analyse sans erreur").program;

    if args.check {
        println!("check ok — no semantic errors.");
        return;
    }

    // ── 4f. Monomorphisation des génériques ───────────────────────────────────
    monomorphize(&mut program);

    // ── 5. Lowering AST → Ocara HIR ────────────────────────────────────────────
    let source_file = args.input.to_string_lossy().to_string();
    let mut ir_module = lower_program_with(&program, &source_file, args.coverage || args.coverage_points);
    if args.coverage_points {
        for (file, line) in ir_module.coverage.iter().flatten() {
            println!("{}\t{}", line, file);
        }
        return;
    }
    ir::coverage::register_points(&mut ir_module);
    if args.leak_check {
        let classes: Vec<String> = program.classes.iter().map(|c| c.name.clone()).collect();
        ir::leak_check::instrument(&mut ir_module, &classes, &core::leak_sites::sites(&program, &args.input));
    }
    if args.profile {
        let classes: Vec<String> = program.classes.iter().map(|c| c.name.clone()).collect();
        ir::profile::instrument(&mut ir_module, &classes);
    }

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
        // Sans liaison, l'objet est la sortie : `-o app.o` tel quel, sinon `<sortie>.o`.
        let obj_path = if args.output.extension().is_some_and(|e| e == "o") { args.output.clone() } else { core::cli::object_path(&args.output) };
        if let Err(e) = fs::write(&obj_path, &obj_bytes) {
            diagnostic::print_error(&args.input, 0, 0, &format!("écriture de '{}': {}", obj_path.display(), e));
            std::process::exit(1);
        }
        println!("objet généré: {}", obj_path.display());
        return;
    }

    // ── 8. Liaison finale ─────────────────────────────────────────────────────
    let obj_path = core::cli::object_path(&args.output);
    // Ne lier libocara_runtime_tauri.a + GTK/WebKit que si le programme importe
    // réellement ocara.Tauri (voir la doc dans src/codegen/link.rs).
    let needs_tauri = ir_module.imports.iter().any(|m| m == "Tauri");
    let needs_sdl = ir_module.imports.iter().any(|m| m == "SDL");
    let needs_image = ir_module.imports.iter().any(|m| m == "Image");

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

        if needs_image {
            diagnostic::print_error(&args.input, 0, 0,
                "ocara.Image n'est pas encore disponible sur Android (voir docs/roadmap.d/builtins-image.md)");
            std::process::exit(1);
        }

        let target = args.target.as_deref().unwrap();
        let runtime_lib = args.android_runtime.as_ref().unwrap();
        let runtime_sdl_lib = if needs_sdl { args.android_runtime_sdl.as_deref() } else { None };
        let feature_libs = android_feature_libs(&ir_module.imports, runtime_lib);
        if !feature_libs.is_empty() && args.android_jni_bridge.is_none() {
            diagnostic::print_error(&args.input, 0, 0,
                "les builtins ocara.Android* requièrent --android-jni-bridge (voir `make build-jni-bridge-android`, docs/android.md)");
            std::process::exit(1);
        }
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

        match link_android(&obj_bytes, &obj_path, &args.output, target, &ndk_home, runtime_lib, runtime_sdl_lib, &feature_libs, args.android_jni_bridge.as_deref(), args.release) {
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

    match link(&obj_bytes, &obj_path, &args.output, args.release, needs_tauri, needs_sdl, needs_image) {
        Ok(()) => {
            println!("compilation réussie → {}", args.output.display());
        }
        Err(e) => {
            diagnostic::print_error(&args.input, 0, 0, &format!("link: {}", e));
            std::process::exit(1);
        }
    }
}

/// `ocara fichier.oc.ui` : valide l'interface (schéma `.oc.ui`) ; `--dump`
/// affiche l'arbre JSON normalisé qu'embarque le binaire, `--preview` écrit
/// une page HTML qui l'affiche avec le moteur de rendu desktop.
fn check_ui_file(args: &core::cli::CliArgs) {
    match core::ui_schema::check_file(&args.input) {
        Ok(spec) if args.preview => {
            let page = core::ui_schema::preview::page(&spec);
            let out = core::ui_schema::preview::output_path(&args.input, &args.output);
            if let Err(e) = fs::write(&out, page) {
                diagnostic::print_error(&out, 0, 0, &format!("écriture de '{}': {}", out.display(), e));
                std::process::exit(1);
            }
            println!("aperçu : file://{}", fs::canonicalize(&out).unwrap_or(out).display());
        }
        Ok(spec) if args.dump => println!("{}", serde_json::to_string_pretty(&spec.tree).unwrap_or_default()),
        Ok(spec) => println!("check ok — {} id, {} action(s).", spec.ids.len(), spec.actions.len()),
        Err(diagnostics) => {
            for d in &diagnostics {
                d.print();
            }
            std::process::exit(1);
        }
    }
}
