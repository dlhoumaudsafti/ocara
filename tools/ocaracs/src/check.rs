// ─────────────────────────────────────────────────────────────────────────────
// Analyse d'un fichier : une ligne `fichier:ligne:col: warning: message` par
// écart (format GCC/clang, cliquable dans VS Code).
// ─────────────────────────────────────────────────────────────────────────────

use std::{io::IsTerminal, path::Path};

use crate::config::Config;
use crate::decls::declarations;
use crate::text::{comment_pos, indent_rule, leading_ws, line_states};

const YELLOW: &str = "\x1b[33m";
const BOLD:   &str = "\x1b[1m";
const RESET:  &str = "\x1b[0m";

fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal()
}

pub fn emit(path: &Path, line: usize, col: usize, msg: &str) {
    let c   = use_color();
    let loc = format!("{}:{}:{}", path.display(), line, col);
    let loc_s = if c { format!("{}{}{}", BOLD, loc, RESET) } else { loc };
    let kw    = if c { format!("{}warning{}", YELLOW, RESET) } else { "warning".into() };
    eprintln!("{}: {}: {}", loc_s, kw, msg);
}

/// Problème d'indentation de `lead` : type attendu (`tabs`) et largeur
/// d'un niveau (`gap`, 0 = non vérifiée).
fn indent_issue(lead: &str, tabs: bool, gap: usize) -> Option<String> {
    let use_tabs = lead.contains('\t');
    if use_tabs != tabs {
        return Some(format!(
            "indentation incohérente : {} attendu(s), {} trouvé(s)",
            if tabs { "tabulations" } else { "espaces" },
            if use_tabs { "tabulations" } else { "espaces" },
        ));
    }
    (gap > 0 && lead.len() % gap != 0).then(|| format!(
        "indentation incohérente : multiple de {} {} attendu",
        gap, if tabs { "tabulation(s)" } else { "espace(s)" }
    ))
}

/// Position du `=` d'une déclaration `var`/`scoped`/`const` (hors `==`, `!=`,
/// `<=`, `>=`, `=>`), s'il y en a un.
pub fn decl_assign_pos(line: &str) -> Option<usize> {
    let t = line.trim_start();
    if !["var ", "scoped ", "const "].iter().any(|kw| t.starts_with(kw)) { return None; }
    let bytes = line.as_bytes();
    (0..bytes.len()).find(|&j| {
        let prev = if j > 0 { bytes[j - 1] } else { 0 };
        let next = bytes.get(j + 1).copied().unwrap_or(0);
        bytes[j] == b'=' && !(next == b'=' || next == b'>' || matches!(prev, b'!' | b'<' | b'>' | b'='))
    })
}

pub fn check_file(path: &Path, content: &str, cfg: &Config) -> usize {
    let lines: Vec<&str> = content.lines().collect();
    let states = line_states(&lines);
    let indent = indent_rule(&lines, &states, cfg);
    let mut found: Vec<(usize, usize, String)> = Vec::new();
    let mut warn = |line: usize, col: usize, msg: &str| found.push((line, col, msg.to_string()));
    let mut blanks = 0usize;

    for (i, (line, state)) in lines.iter().zip(&states).enumerate() {
        let lnum = i + 1;
        let in_bt = state.inside_bt();

        // ── R01 : indentation (type R19, largeur R20) ─────────────────────
        if let Some((tabs, gap)) = indent {
            let lead = leading_ws(line);
            if !in_bt && !lead.is_empty() {
                if let Some(msg) = indent_issue(lead, tabs, gap) { warn(lnum, 1, &msg); }
            }
        }

        // ── R02 : ligne vide sans whitespace ──────────────────────────────
        if cfg.empty_line_ws && !in_bt && !line.is_empty() && line.trim().is_empty() {
            warn(lnum, 1, "ligne vide contient des espaces ou tabulations");
        }

        // ── R03 : espaces autour de '=' dans les déclarations ─────────────
        if cfg.spacing_assign && !state.starts_in_bt {
            if let Some(j) = decl_assign_pos(line) {
                let bytes = line.as_bytes();
                if j == 0 || !matches!(bytes[j - 1], b' ' | b'\t') { warn(lnum, j + 1, "espace manquant avant '='"); }
                if bytes.get(j + 1).is_some_and(|b| !matches!(b, b' ' | b'\t')) { warn(lnum, j + 2, "espace manquant après '='"); }
            }
        }

        // ── R04 : pas de whitespace en fin de ligne ───────────────────────
        if cfg.trailing_ws && !state.ends_in_bt && !line.is_empty() {
            let trimmed = line.trim_end();
            if trimmed.len() < line.len() {
                warn(lnum, trimmed.len() + 1, "espace(s) ou tabulation(s) en fin de ligne");
            }
        }

        // ── R05 : longueur de ligne ───────────────────────────────────────
        let len = line.chars().count();
        if cfg.max_line_length > 0 && len > cfg.max_line_length {
            warn(lnum, cfg.max_line_length + 1, &format!("ligne trop longue : {} caractères (max {})", len, cfg.max_line_length));
        }

        // ── R06 : lignes vides consécutives ───────────────────────────────
        if cfg.blank_lines_max > 0 {
            if line.trim().is_empty() && !in_bt {
                blanks += 1;
                if blanks > cfg.blank_lines_max {
                    warn(lnum, 1, &format!("trop de lignes vides consécutives (max {})", cfg.blank_lines_max));
                }
            } else {
                blanks = 0;
            }
        }

        // ── R10 : espace après '//' ───────────────────────────────────────
        if cfg.comment_spacing && !in_bt {
            if let Some(pos) = comment_pos(line, *state) {
                let after = &line[pos + 2..];
                if !after.is_empty() && !after.starts_with(' ') && !after.starts_with('/') {
                    warn(lnum, pos + 1, "espace manquant après '//' dans le commentaire");
                }
            }
        }
    }

    // ── R07/R08/R09/R12/R13 : nommage ────────────────────────────────────────
    for decl in declarations(&path.to_string_lossy(), &lines, &states, cfg) {
        if let Some(msg) = decl.issue() { warn(decl.line, 1, &msg); }
    }

    // ── R11 : le fichier se termine par une newline ──────────────────────────
    if cfg.file_ends_newline && !content.is_empty() && !content.ends_with('\n') {
        warn(lines.len(), lines.last().map_or(0, |l| l.len()) + 1, "le fichier ne se termine pas par une newline");
    }

    found.sort_by_key(|(line, col, _)| (*line, *col));
    for (line, col, msg) in &found { emit(path, *line, *col, msg); }
    found.len()
}
