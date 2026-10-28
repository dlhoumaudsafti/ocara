use std::path::{Path, PathBuf};

/// Arguments de la ligne de commande
#[derive(Debug)]
pub struct CliArgs {
    pub input:   PathBuf,
    pub output:  PathBuf,
    /// true = afficher les tokens + HIR sans compiler
    pub dump:    bool,
    /// true = s'arrêter après l'analyse sémantique
    pub check:   bool,
    /// true = aperçu HTML d'un `.oc.ui` (src/core/ui_schema.d/preview.rs)
    pub preview: bool,
    /// true = produire le fichier .o mais ne pas linker
    pub no_link: bool,
    /// true = strip les symboles du binaire produit (via le linker)
    pub release: bool,
    /// true = fonctions instrumentées pour le profilage (src/ir/profile.rs)
    pub profile: bool,
    /// true = allocations comptées, bilan des fuites à la sortie (src/ir/leak_check.rs)
    pub leak_check: bool,
    /// true = couverture par ligne, écrite à la sortie dans `$OCARA_COVERAGE_FILE` (src/ir/coverage.rs)
    pub coverage: bool,
    /// true = vérifie seulement l'architecture hexagonale du programme
    pub check_architecture: bool,
    /// true = liste les points de couverture (`ligne\tfichier`) sans compiler
    pub coverage_points: bool,
    /// true = rapport d'architecture du projet en JSON (core::analysis::architecture_report)
    pub architecture_report: bool,
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
    /// Constantes de build imposées (`--when System::OS=android`) : variantes
    /// `when` d'une autre cible compilées pour l'hôte (ocaraunit, RuntimeTest.oc).
    pub when: Vec<(String, String)>,
    /// true = serveur de langage (LSP) sur stdin/stdout, sans fichier d'entrée
    pub lsp: bool,
}

pub fn print_help() {
    println!("Ocara — Object Code Abstraction Runtime Architecture v{}", env!("CARGO_PKG_VERSION"));
    println!("Un langage de programmation simple avec un compilateur écrit en Rust.");
    println!("Auteur : David Lhoumaud");
    println!();
    println!("Usage :");
    println!("  ocara <fichier.oc> [options]");
    println!("  ocara <fichier.oc.ui> [--dump]   Valide une interface .oc.ui (--dump : arbre JSON normalisé)");
    println!("  ocara <fichier.oc.ui> --preview [-o page.html]");
    println!("                Aperçu de l'interface dans un navigateur (moteur de rendu desktop)");
    println!();
    println!("Options :");
    println!("  -o <sortie>   Fichier de sortie (défaut : out)");
    println!("  --src <dir>   Répertoire racine pour la résolution des imports");
    println!("  --check       Analyse sémantique uniquement, sans compilation");
    println!("  --dump        Affiche les tokens, l'AST et la représentation intermédiaire (HIR)");
    println!("  --no-link     Produit le fichier .o sans linker");
    println!("  --release     Build release (System::BUILD = \"release\", symboles retirés)");
    println!("  --profile     Mesure le temps de chaque fonction ; rapport à la sortie du programme");
    println!("                (fichier OCARA_PROFILE_OUT, sinon stderr)");
    println!("  --leak-check  Compte allocations et libérations ; valeurs non libérées listées à la sortie");
    println!("  --coverage    Couverture par ligne, écrite à la sortie dans $OCARA_COVERAGE_FILE");
    println!("  --check-architecture  Vérifie seulement l'architecture hexagonale (imports, wiring)");
    println!("  --coverage-points     Liste les lignes instrumentées par --coverage (ligne<TAB>fichier), sans compiler");
    println!("  --architecture-report Rapport d'architecture du projet en JSON (type, contextes, couches, violations)");
    println!("                (fichier OCARA_LEAK_OUT, sinon stderr)");
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
    println!("  --when <System::NOM=valeur>");
    println!("                Impose une constante de build aux clauses `when` (OS, ARCH, BUILD),");
    println!("                répétable. Ex. : --when System::OS=android sur l'hôte.");
    println!("  --android-ndk <dir>");
    println!("                Racine du NDK Android (défaut : $ANDROID_NDK_HOME).");
    println!("  --lsp         Serveur de langage (LSP) sur stdin/stdout, pour les éditeurs");
    println!("  -h, --help    Affiche cette aide");
    println!();
    println!("Exemples :");
    println!("  ocara main.oc -o ./mon_programme");
    println!("  ocara main.oc --check");
    println!("  ocara garage.oc.ui");
    println!("  ocara tests/mainTest.oc --src .");
    println!("  ocara main.oc --target aarch64-linux-android --no-link -o out");
    println!("  ocara main.oc --target aarch64-linux-android --android-runtime libocara_runtime.a -o libmain.so");
}

/// Code de sortie d'une ligne de commande invalide.
const EXIT_USAGE: i32 = 2;

pub fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        std::process::exit(0);
    }
    if args.is_empty() {
        print_help();
        std::process::exit(EXIT_USAGE);
    }
    parse(&args).unwrap_or_else(|message| {
        eprintln!("ocara : {}", message);
        eprintln!("Voir : ocara --help");
        std::process::exit(EXIT_USAGE);
    })
}

/// Analyse les arguments (sans le nom du programme). Option inconnue, option
/// sans sa valeur, second fichier d'entrée, fichier manquant ou sortie qui
/// écraserait le source : erreur, jamais un repli silencieux. Un premier
/// argument `build` (forme historique `ocara build main.oc`) est ignoré.
pub fn parse(args: &[String]) -> Result<CliArgs, String> {
    let mut input: Option<PathBuf> = None;
    let mut output  = PathBuf::from("out");
    let mut dump    = false;
    let mut check   = false;
    let mut preview = false;
    let mut no_link = false;
    let mut release = false;
    let mut profile = false;
    let mut leak_check = false;
    let mut coverage = false;
    let mut check_architecture = false;
    let mut coverage_points = false;
    let mut architecture_report = false;
    let mut src_dir = None;
    let mut target: Option<String> = None;
    let mut android_runtime: Option<PathBuf> = None;
    let mut android_runtime_sdl: Option<PathBuf> = None;
    let mut android_jni_bridge: Option<PathBuf> = None;
    let mut android_ndk: Option<PathBuf> = None;
    let mut lsp = false;
    let mut when = Vec::new();

    let skip_build = args.first().is_some_and(|a| a == "build");
    let mut rest = args.iter().skip(usize::from(skip_build));
    while let Some(arg) = rest.next() {
        let mut value = |name: &str| rest.next().cloned().ok_or_else(|| format!("{} nécessite une valeur", name));
        match arg.as_str() {
            "--dump"    => dump    = true,
            "--check"   => check   = true,
            "--preview" => preview = true,
            "--no-link" => no_link = true,
            "--release" => release = true,
            "--profile" => profile = true,
            "--leak-check" => leak_check = true,
            "--coverage" => coverage = true,
            "--check-architecture" => check_architecture = true,
            "--coverage-points" => coverage_points = true,
            "--architecture-report" => architecture_report = true,
            "--lsp"     => lsp     = true,
            "-o" => output = PathBuf::from(value("-o")?),
            "--src" => src_dir = Some(PathBuf::from(value("--src")?)),
            "--target" => target = Some(value("--target")?),
            "--android-runtime" => android_runtime = Some(PathBuf::from(value("--android-runtime")?)),
            "--android-runtime-sdl" => android_runtime_sdl = Some(PathBuf::from(value("--android-runtime-sdl")?)),
            "--android-jni-bridge" => android_jni_bridge = Some(PathBuf::from(value("--android-jni-bridge")?)),
            "--android-ndk" => android_ndk = Some(PathBuf::from(value("--android-ndk")?)),
            "--when" => when.push(parse_when(&value("--when")?)?),
            option if option.starts_with('-') => return Err(format!("option inconnue : {}", option)),
            file => {
                if let Some(first) = &input {
                    return Err(format!("un seul fichier d'entrée attendu (reçu : {} et {})", first.display(), file));
                }
                input = Some(PathBuf::from(file));
            }
        }
    }
    let input = match input {
        Some(input) => input,
        None if lsp => PathBuf::new(),
        None => return Err("aucun fichier d'entrée (.oc)".to_string()),
    };
    if !lsp && same_path(&input, &output) {
        return Err(format!("la sortie -o {} est le fichier source lui-même : il serait écrasé", output.display()));
    }
    Ok(CliArgs { input, output, dump, check, preview, no_link, release, profile, leak_check, coverage, check_architecture, coverage_points, architecture_report, src_dir, target, android_runtime, android_runtime_sdl, android_jni_bridge, android_ndk, when, lsp })
}

/// `System::OS=android` (ou `OS=android`) → (`OS`, `android`), constante et
/// valeur prises dans `parsing::build::DOMAINS`.
fn parse_when(raw: &str) -> Result<(String, String), String> {
    let (name, value) = raw.split_once('=').ok_or_else(|| format!("--when {} : forme attendue System::NOM=valeur", raw))?;
    let name = name.trim().trim_start_matches("System::");
    let value = value.trim().trim_matches(|c| c == '\'' || c == '"');
    let (_, domain) = crate::parsing::build::DOMAINS.iter().find(|(n, _)| *n == name)
        .ok_or_else(|| format!("--when {} : constante inconnue (OS, ARCH ou BUILD)", raw))?;
    if !domain.contains(&value) {
        return Err(format!("--when {} : valeur inconnue pour System::{} ({})", raw, name, domain.join(", ")));
    }
    Ok((name.to_string(), value.to_string()))
}

/// Même fichier, existant ou non (`./x.oc` et `x.oc`).
fn same_path(a: &Path, b: &Path) -> bool {
    let absolute = |p: &Path| p.canonicalize().unwrap_or_else(|_| {
        let joined = std::env::current_dir().map(|cwd| cwd.join(p)).unwrap_or_else(|_| p.to_path_buf());
        joined.components().collect()
    });
    absolute(a) == absolute(b)
}

/// Fichier objet intermédiaire de `output` : `<output>.o` (jamais l'extension
/// remplacée — `-o build/app.1.2` donnait `build/app.1.o`).
pub fn object_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_os_string();
    name.push(".o");
    PathBuf::from(name)
}
