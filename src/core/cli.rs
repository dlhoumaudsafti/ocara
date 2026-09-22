use std::path::PathBuf;

/// Arguments de la ligne de commande
#[derive(Debug)]
pub struct CliArgs {
    pub input:   PathBuf,
    pub output:  PathBuf,
    /// true = afficher les tokens + HIR sans compiler
    pub dump:    bool,
    /// true = s'arrêter après l'analyse sémantique
    pub check:   bool,
    /// true = produire le fichier .o mais ne pas linker
    pub no_link: bool,
    /// true = strip les symboles du binaire produit (via le linker)
    pub release: bool,
    /// Répertoire racine pour la résolution des imports (défaut : répertoire du fichier d'entrée)
    pub src_dir: Option<PathBuf>,
    /// Triple cible Cranelift (ex: "aarch64-linux-android") — quand présent,
    /// le codegen utilise `isa::lookup(triple)` au lieu de `cranelift_native::builder()`
    /// (voir docs/roadmap.d/packaging-android.md, sous-chantier 1).
    ///
    /// La liaison finale pour une cible croisée n'est supportée QUE pour un
    /// triple Android quand `--android-runtime` est fourni (sous-chantier 2) —
    /// dans tout autre cas (autre cible croisée, ou Android sans runtime
    /// fourni), `--target` exige `--no-link`.
    pub target: Option<String>,
    /// Chemin vers un `libocara_runtime.a` pré-compilé pour la cible Android
    /// visée par `--target` (ex: produit par `make build-runtime-android`,
    /// voir docs/roadmap.d/packaging-android.md sous-chantier 3). Contrairement
    /// au runtime de l'hôte (embarqué dans `ocara` via `include_bytes!` à sa
    /// propre compilation), le runtime Android n'est PAS embarqué : le binaire
    /// `ocara` reste un binaire hôte ordinaire, buildable sans le NDK.
    pub android_runtime: Option<PathBuf>,
    /// Chemin vers un `libocara_runtime_sdl.a` pré-compilé pour la cible
    /// Android visée (produit par `make build-runtime-sdl-android`) — requis
    /// UNIQUEMENT si le programme importe `ocara.SDL` (voir sous-chantier 4,
    /// docs/roadmap.d/packaging-android.md).
    pub android_runtime_sdl: Option<PathBuf>,
    /// Chemin vers un `libocara_runtime_android_jni.a` pré-compilé (crate
    /// `runtime_android_jni`) — indépendant de tout import Ocara, requis
    /// seulement si le `.so` produit doit être chargeable par une Activity
    /// Android via JNI (voir docs/roadmap.d/packaging-android-webview-hybrid.md).
    pub android_jni_bridge: Option<PathBuf>,
    /// Répertoire racine du NDK Android (`$ANDROID_NDK_HOME` si absent) —
    /// requis avec `--android-runtime` pour localiser le clang de croisement.
    pub android_ndk: Option<PathBuf>,
}

pub fn print_help() {
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
    println!("  --target <triple>");
    println!("                Cible de compilation croisée Cranelift (ex: aarch64-linux-android).");
    println!("                Sans --android-runtime, exige --no-link (voir docs/roadmap.d/packaging-android.md).");
    println!("  --android-runtime <fichier.a>");
    println!("                libocara_runtime.a pré-compilé pour la cible --target (Android uniquement).");
    println!("                Active la liaison finale (.so) au lieu d'exiger --no-link.");
    println!("  --android-runtime-sdl <fichier.a>");
    println!("                libocara_runtime_sdl.a pré-compilé — requis seulement si le programme");
    println!("                importe ocara.SDL.");
    println!("  --android-jni-bridge <fichier.a>");
    println!("                libocara_runtime_android_jni.a pré-compilé — requis seulement si le .so");
    println!("                doit être chargeable par une Activity Android via JNI.");
    println!("  --android-ndk <dir>");
    println!("                Racine du NDK Android (défaut : $ANDROID_NDK_HOME).");
    println!("  -h, --help    Affiche cette aide");
    println!();
    println!("Exemples :");
    println!("  ocara main.oc -o ./mon_programme");
    println!("  ocara main.oc --check");
    println!("  ocara tests/mainTest.oc --src .");
    println!("  ocara main.oc --target aarch64-linux-android --no-link -o out");
    println!("  ocara main.oc --target aarch64-linux-android --android-runtime libocara_runtime.a -o libmain.so");
}

pub fn parse_args() -> CliArgs {
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
    let mut target: Option<String> = None;
    let mut android_runtime: Option<PathBuf> = None;
    let mut android_runtime_sdl: Option<PathBuf> = None;
    let mut android_jni_bridge: Option<PathBuf> = None;
    let mut android_ndk: Option<PathBuf> = None;

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
            "--target" if i + 1 < args.len() => {
                target = Some(args[i + 1].clone());
                i += 1;
            }
            "--android-runtime" if i + 1 < args.len() => {
                android_runtime = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--android-runtime-sdl" if i + 1 < args.len() => {
                android_runtime_sdl = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--android-jni-bridge" if i + 1 < args.len() => {
                android_jni_bridge = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--android-ndk" if i + 1 < args.len() => {
                android_ndk = Some(PathBuf::from(&args[i + 1]));
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
    CliArgs { input, output, dump, check, no_link, release, src_dir, target, android_runtime, android_runtime_sdl, android_jni_bridge, android_ndk }
}
