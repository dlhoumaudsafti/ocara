// ─────────────────────────────────────────────────────────────────────────────
// diagnostic.rs  –  formatage unifié des erreurs et avertissements Ocara
//
// Format de sortie (compatible VS Code / GCC / clang) :
//   <bold>path/au/fichier.oc:LINE:COL:</bold> <red>error:</red> message
//   <bold>path/au/fichier.oc:LINE:COL:</bold> <cyan>init</cyan>.<red>ERROR</red> message (avec contexte runtime)
//   <bold>path/au/fichier.oc:LINE:COL:</bold> <yellow>warning:</yellow> message
//   <bold>path/au/fichier.oc:LINE:COL:</bold> <cyan>exit</cyan>.<yellow>WARNING</yellow> message (avec contexte runtime)
//
// VS Code reconnaît automatiquement ce format dans le terminal intégré
// et rend chaque ligne cliquable (ouvre le fichier à la bonne ligne).
// ─────────────────────────────────────────────────────────────────────────────

use std::io::IsTerminal;

// ── Codes ANSI ────────────────────────────────────────────────────────────────

const RED:   &str = "\x1b[31m";
const YELLOW:&str = "\x1b[33m";
const CYAN:  &str = "\x1b[36m";  // Couleur info pour le contexte runtime
const BOLD:  &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Vrai si stderr supporte les séquences ANSI (tty et pas NO_COLOR).
fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal()
}

// ── Helpers internes ──────────────────────────────────────────────────────────

fn bold(s: &str, color: bool) -> String {
    if color { format!("{}{}{}", BOLD, s, RESET) } else { s.to_owned() }
}

fn colored(s: &str, code: &str, color: bool) -> String {
    if color { format!("{}{}{}", code, s, RESET) } else { s.to_owned() }
}

// ── API publique ──────────────────────────────────────────────────────────────

/// Affiche une ligne d'erreur au format GCC/clang sur stderr.
///
/// ```text
/// path/fichier.oc:5:46: error: symbole indéfini 'foo'
/// path/fichier.oc:5:46: init.ERROR symbole indéfini 'foo' (avec contexte runtime)
/// ```
#[allow(dead_code)]
pub fn print_error(path: &std::path::Path, line: usize, col: usize, msg: &str) {
    print_error_ctx(path, line, col, msg, None);
}

/// Affiche une ligne d'erreur avec contexte runtime (ex: "init", "main", "error", "success", "exit")
pub fn print_error_ctx(path: &std::path::Path, line: usize, col: usize, msg: &str, runtime_ctx: Option<&str>) {
    let c    = use_color();
    let loc  = bold(&format!("{}:{}:{}", path.display(), line, col), c);
    
    if let Some(ctx) = runtime_ctx {
        let ctx_colored = colored(ctx, CYAN, c);
        let level = colored("ERROR", RED, c);
        eprintln!("{}: {}.{} {}", loc, ctx_colored, level, msg);
    } else {
        let kw = colored("error", RED, c);
        eprintln!("{}: {}: {}", loc, kw, msg);
    }
}

/// Affiche une ligne d'avertissement au format GCC/clang sur stderr.
///
/// ```text
/// path/fichier.oc:6:5: warning: variable 'truc' jamais utilisée
/// path/fichier.oc:6:5: exit.WARNING variable 'truc' jamais utilisée (avec contexte runtime)
/// ```
#[allow(dead_code)]
pub fn print_warn(path: &std::path::Path, line: usize, col: usize, msg: &str) {
    print_warn_ctx(path, line, col, msg, None);
}

/// Affiche une ligne d'avertissement avec contexte runtime
pub fn print_warn_ctx(path: &std::path::Path, line: usize, col: usize, msg: &str, runtime_ctx: Option<&str>) {
    let c   = use_color();
    let loc = bold(&format!("{}:{}:{}", path.display(), line, col), c);
    
    if let Some(ctx) = runtime_ctx {
        let ctx_colored = colored(ctx, CYAN, c);
        let level = colored("WARNING", YELLOW, c);
        eprintln!("{}: {}.{} {}", loc, ctx_colored, level, msg);
    } else {
        let kw = colored("warning", YELLOW, c);
        eprintln!("{}: {}: {}", loc, kw, msg);
    }
}
