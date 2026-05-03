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
    println!("  -h, --help    Affiche cette aide");
    println!();
    println!("Exemples :");
    println!("  ocara main.oc -o ./mon_programme");
    println!("  ocara main.oc --check");
    println!("  ocara tests/mainTest.oc --src .");
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
