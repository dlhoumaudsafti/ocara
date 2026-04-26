// ─────────────────────────────────────────────────────────────────────────────
// diagnostic.rs  –  formatage unifié des erreurs et avertissements Ocara
//
// Format de sortie (compatible VS Code / GCC / clang) :
//   <bold>path/au/fichier.oc:LINE:COL:</bold> <red>error:</red> message
//   <bold>path/au/fichier.oc:LINE:COL:</bold> <yellow>warning:</yellow> message
//
// VS Code reconnaît automatiquement ce format dans le terminal intégré
// et rend chaque ligne cliquable (ouvre le fichier à la bonne ligne).
// ─────────────────────────────────────────────────────────────────────────────

use std::io::IsTerminal;

// ── Codes ANSI ────────────────────────────────────────────────────────────────

const RED:   &str = "\x1b[31m";
const YELLOW:&str = "\x1b[33m";
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
/// ```
pub fn print_error(path: &std::path::Path, line: usize, col: usize, msg: &str) {
    let c    = use_color();
    let loc  = bold(&format!("{}:{}:{}", path.display(), line, col), c);
    let kw   = colored("error", RED, c);
    eprintln!("{}: {}: {}", loc, kw, msg);
}

/// Affiche une ligne d'avertissement au format GCC/clang sur stderr.
///
/// ```text
/// path/fichier.oc:6:5: warning: variable 'truc' jamais utilisée
/// ```
pub fn print_warn(path: &std::path::Path, line: usize, col: usize, msg: &str) {
    let c   = use_color();
    let loc = bold(&format!("{}:{}:{}", path.display(), line, col), c);
    let kw  = colored("warning", YELLOW, c);
    eprintln!("{}: {}: {}", loc, kw, msg);
}
