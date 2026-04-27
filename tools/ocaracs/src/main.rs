// ─────────────────────────────────────────────────────────────────────────────
// ocaracs — analyseur de style pour Ocara
//
// Usage :
//   ocaracs <fichier.oc>
//   ocaracs <dossier>
//
// Configuration : fichier .ocaracs à la racine du projet (TOML simplifié).
//
// Format de sortie (compatible VS Code / GCC / clang, cliquable) :
//   fichier.oc:LIGNE:COL: warning: message
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    collections::HashSet,
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
};

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Config {
    /// R01 — cohérence de l'indentation (première ligne indentée = référence)
    indent:            bool,
    /// R02 — pas d'espaces ou tabulations sur les lignes vides
    empty_line_ws:     bool,
    /// R03 — espaces autour de '=' dans var / scoped / const
    spacing_assign:    bool,
    /// R04 — pas d'espaces ou tabulations en fin de ligne
    trailing_ws:       bool,
    /// R05 — longueur max d'une ligne (0 = désactivé)
    max_line_length:   usize,
    /// R06 — max lignes vides consécutives (0 = désactivé)
    blank_lines_max:   usize,
    /// R07 — classes en PascalCase
    naming_class:      bool,
    /// R08 — fonctions en camelCase (première lettre minuscule)
    naming_function:   bool,
    /// R09 — constantes en UPPER_SNAKE_CASE
    naming_const:      bool,
    /// R10 — espace après '//' dans les commentaires
    comment_spacing:   bool,
    /// R11 — le fichier se termine par une newline
    file_ends_newline: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            indent:            true,
            empty_line_ws:     true,
            spacing_assign:    true,
            trailing_ws:       true,
            max_line_length:   120,
            blank_lines_max:   2,
            naming_class:      true,
            naming_function:   true,
            naming_const:      true,
            comment_spacing:   true,
            file_ends_newline: true,
        }
    }
}

fn parse_config(content: &str) -> Config {
    let mut cfg = Config::default();
    let mut in_rules = false;
    for raw in content.lines() {
        let line = raw.trim();
        if line == "[rules]"        { in_rules = true;  continue; }
        if line.starts_with('[')    { in_rules = false; continue; }
        if !in_rules || line.starts_with('#') || line.is_empty() { continue; }
        let mut parts = line.splitn(2, '=');
        let key = parts.next().unwrap_or("").trim();
        let val = parts.next().unwrap_or("").split('#').next().unwrap_or("").trim();
        match key {
            "indentation"         => cfg.indent            = val == "true",
            "empty_lines"         => cfg.empty_line_ws     = val == "true",
            "spacing_assign"      => cfg.spacing_assign    = val == "true",
            "trailing_whitespace" => cfg.trailing_ws       = val == "true",
            "max_line_length"     => cfg.max_line_length   = val.parse().unwrap_or(120),
            "blank_lines_max"     => cfg.blank_lines_max   = val.parse().unwrap_or(2),
            "naming_class"        => cfg.naming_class      = val == "true",
            "naming_function"     => cfg.naming_function   = val == "true",
            "naming_const"        => cfg.naming_const      = val == "true",
            "comment_spacing"     => cfg.comment_spacing   = val == "true",
            "file_ends_newline"   => cfg.file_ends_newline = val == "true",
            _ => {}
        }
    }
    cfg
}

fn load_config(root: &Path) -> Config {
    match fs::read_to_string(root.join(".ocaracs")) {
        Ok(c)  => parse_config(&c),
        Err(_) => Config::default(),
    }
}

fn find_project_root(path: &Path) -> PathBuf {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
    };
    let abs = dir.canonicalize().unwrap_or_else(|_| dir.clone());
    let mut cur = abs;
    loop {
        if cur.join(".ocaracs").exists() { return cur; }
        match cur.parent() {
            Some(p) => cur = p.to_path_buf(),
            None    => return dir,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Diagnostics
// ─────────────────────────────────────────────────────────────────────────────

const YELLOW: &str = "\x1b[33m";
const BOLD:   &str = "\x1b[1m";
const RESET:  &str = "\x1b[0m";

fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal()
}

fn emit(path: &Path, line: usize, col: usize, msg: &str) {
    let c   = use_color();
    let loc = format!("{}:{}:{}", path.display(), line, col);
    let loc_s = if c { format!("{}{}{}", BOLD,   loc,       RESET) } else { loc };
    let kw    = if c { format!("{}warning{}", YELLOW, RESET)       } else { "warning".into() };
    eprintln!("{}: {}: {}", loc_s, kw, msg);
}

// ─────────────────────────────────────────────────────────────────────────────
// Détection des lignes entièrement à l'intérieur d'une chaîne backtick
// ─────────────────────────────────────────────────────────────────────────────

/// Retourne pour chaque ligne si elle est ENTIÈREMENT à l'intérieur
/// d'une chaîne backtick multiligne (ni la ligne d'ouverture ni de fermeture).
fn backtick_flags(lines: &[&str]) -> Vec<bool> {
    let mut result  = vec![false; lines.len()];
    let mut in_bt   = false;
    for (i, line) in lines.iter().enumerate() {
        let was_in_bt = in_bt;
        let mut in_str = false;
        let chars: Vec<char> = line.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            let ch      = chars[j];
            let escaped = j > 0 && chars[j - 1] == '\\';
            match ch {
                '"' if !in_bt  && !escaped => in_str = !in_str,
                '`' if !in_str && !escaped => in_bt  = !in_bt,
                _ => {}
            }
            j += 1;
        }
        // La ligne est "dans le backtick" ssi elle était ouverte en début ET en fin de ligne
        result[i] = was_in_bt && in_bt;
    }
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Extraction des imports utilisateur
// ─────────────────────────────────────────────────────────────────────────────

const OCARA_BUILTINS: &[&str] = &[
    "IO", "Math", "String", "Array", "Map",
    "Convert", "System", "Regex", "HTTPRequest", "HTTPServer", "Thread", "Mutex",
    "DateTime", "Date", "Time", "UnitTest",
];

fn extract_user_imports(content: &str, file_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if !t.starts_with("import ") { continue; }
        // Supprimer la partie " as Alias" éventuelle
        let rest_raw = t[7..].trim();
        let rest = rest_raw.split(" as ").next().unwrap_or(rest_raw).trim();
        if rest.starts_with("ocara.") || rest == "ocara.*" { continue; }
        let first = rest.split('.').next().unwrap_or("");
        if OCARA_BUILTINS.contains(&first) { continue; }
        let mut path = file_dir.to_path_buf();
        for seg in rest.split('.') { path.push(seg); }
        path.set_extension("oc");
        out.push(path);
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers nommage
// ─────────────────────────────────────────────────────────────────────────────

fn is_pascal_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
        && s.chars().all(|c| c.is_alphanumeric())
}

fn starts_lowercase(s: &str) -> bool {
    s.chars().next().map(|c| c.is_lowercase()).unwrap_or(false)
}

fn is_upper_snake(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('_')
        && s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Trouve la position du premier '//' hors d'une chaîne "..."
fn find_comment_pos(line: &str) -> Option<usize> {
    let bytes  = line.as_bytes();
    let mut in_str = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_str = !in_str;
        }
        if !in_str && i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            return Some(i);
        }
        i += 1;
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Analyse d'un fichier
// ─────────────────────────────────────────────────────────────────────────────

fn check_file(path: &Path, content: &str, cfg: &Config) -> usize {
    let lines: Vec<&str> = content.lines().collect();
    let bt = backtick_flags(&lines);

    // Détecter l'unité d'indentation : première ligne indentée hors backtick
    let indent_unit: Option<String> = if cfg.indent {
        lines.iter().enumerate()
            .filter(|(i, l)| !bt.get(*i).copied().unwrap_or(false) && !l.is_empty())
            .find_map(|(_, l)| {
                let lead: String = l.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
                if lead.is_empty() { None } else { Some(lead) }
            })
    } else {
        None
    };

    let mut count  = 0usize;
    let mut blanks = 0usize;

    for (i, line) in lines.iter().enumerate() {
        let lnum      = i + 1;
        let in_bt     = bt.get(i).copied().unwrap_or(false);

        // ── R01 : Cohérence de l'indentation ─────────────────────────────
        if cfg.indent {
            if let Some(ref unit) = indent_unit {
                if !in_bt && !line.is_empty() {
                    let lead: String = line.chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .collect();
                    if !lead.is_empty() {
                        let use_tabs  = lead.contains('\t');
                        let unit_tabs = unit.contains('\t');
                        if use_tabs != unit_tabs {
                            emit(path, lnum, 1, &format!(
                                "indentation incohérente : {} attendu(s), {} trouvé(s)",
                                if unit_tabs { "tabulations" } else { "espaces" },
                                if use_tabs  { "tabulations" } else { "espaces" },
                            ));
                            count += 1;
                        } else if !use_tabs {
                            let unit_len = unit.len();
                            if lead.len() % unit_len != 0 {
                                emit(path, lnum, 1, &format!(
                                    "indentation incohérente : multiple de {} espace(s) attendu",
                                    unit_len
                                ));
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        // ── R02 : Ligne vide sans whitespace ──────────────────────────────
        if cfg.empty_line_ws && !in_bt {
            if !line.is_empty() && line.chars().all(|c| c == ' ' || c == '\t') {
                emit(path, lnum, 1, "ligne vide contient des espaces ou tabulations");
                count += 1;
            }
        }

        // ── R03 : Espaces autour de '=' dans les déclarations ─────────────
        if cfg.spacing_assign && !in_bt {
            let trimmed = line.trim();
            if trimmed.starts_with("var ")
                || trimmed.starts_with("scoped ")
                || trimmed.starts_with("const ")
            {
                let bytes = line.as_bytes();
                for j in 0..bytes.len() {
                    if bytes[j] != b'=' { continue; }
                    let prev = if j > 0              { bytes[j - 1] } else { 0 };
                    let next = if j + 1 < bytes.len(){ bytes[j + 1] } else { 0 };
                    // Ignorer ==  !=  <=  >=  =>
                    if next == b'=' || prev == b'!' || prev == b'<'
                        || prev == b'>' || prev == b'=' || next == b'>'
                    {
                        continue;
                    }
                    if prev != b' ' && prev != b'\t' {
                        emit(path, lnum, j + 1, "espace manquant avant '='");
                        count += 1;
                    }
                    if next != b' ' && next != b'\t' && next != b'\n' && next != 0 {
                        emit(path, lnum, j + 2, "espace manquant après '='");
                        count += 1;
                    }
                    break; // premier '=' de la déclaration uniquement
                }
            }
        }

        // ── R04 : Pas de whitespace en fin de ligne ────────────────────────
        if cfg.trailing_ws && !in_bt && !line.is_empty() {
            let trimmed_r = line.trim_end();
            if trimmed_r.len() < line.len() {
                emit(path, lnum, trimmed_r.len() + 1,
                    "espace(s) ou tabulation(s) en fin de ligne");
                count += 1;
            }
        }

        // ── R05 : Longueur de ligne ────────────────────────────────────────
        if cfg.max_line_length > 0 {
            let len = line.chars().count();
            if len > cfg.max_line_length {
                emit(path, lnum, cfg.max_line_length + 1, &format!(
                    "ligne trop longue : {} caractères (max {})",
                    len, cfg.max_line_length
                ));
                count += 1;
            }
        }

        // ── R06 : Lignes vides consécutives ───────────────────────────────
        if cfg.blank_lines_max > 0 {
            if line.trim().is_empty() {
                blanks += 1;
                if blanks > cfg.blank_lines_max {
                    emit(path, lnum, 1, &format!(
                        "trop de lignes vides consécutives (max {})",
                        cfg.blank_lines_max
                    ));
                    count += 1;
                }
            } else {
                blanks = 0;
            }
        }

        // ── R07 : Nommage des classes (PascalCase) ─────────────────────────
        if cfg.naming_class && !in_bt {
            let t = line.trim();
            if t.starts_with("class ") {
                let rest = t[6..].trim();
                let name = rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next().unwrap_or("");
                if !name.is_empty() && !is_pascal_case(name) {
                    emit(path, lnum, 1, &format!(
                        "classe '{}' devrait être en PascalCase", name
                    ));
                    count += 1;
                }
            }
        }

        // ── R08 : Nommage des fonctions (camelCase / minuscule) ────────────
        if cfg.naming_function && !in_bt {
            let t = line.trim();
            if t.starts_with("function ") {
                let rest = t[9..].trim();
                let name = rest.split(|c: char| c == '(' || c == ':' || c == ' ')
                    .next().unwrap_or("");
                if !name.is_empty() && !starts_lowercase(name) {
                    emit(path, lnum, 1, &format!(
                        "fonction '{}' devrait commencer par une minuscule (camelCase)",
                        name
                    ));
                    count += 1;
                }
            }
        }

        // ── R09 : Nommage des constantes (UPPER_SNAKE_CASE) ────────────────
        if cfg.naming_const && !in_bt {
            let t = line.trim();
            if t.starts_with("const ") {
                let rest = t[6..].trim();
                let name = rest.split(|c: char| c == ' ' || c == '=' || c == ':')
                    .next().unwrap_or("");
                if !name.is_empty() && !is_upper_snake(name) {
                    emit(path, lnum, 1, &format!(
                        "constante '{}' devrait être en UPPER_SNAKE_CASE", name
                    ));
                    count += 1;
                }
            }
        }

        // ── R10 : Espace après '//' ────────────────────────────────────────
        if cfg.comment_spacing && !in_bt {
            if let Some(pos) = find_comment_pos(line) {
                let after = &line[pos + 2..];
                if !after.is_empty() && !after.starts_with(' ') && !after.starts_with('/') {
                    emit(path, lnum, pos + 1,
                        "espace manquant après '//' dans le commentaire");
                    count += 1;
                }
            }
        }
    }

    // ── R11 : Fichier se termine par une newline ──────────────────────────────
    if cfg.file_ends_newline && !content.ends_with('\n') {
        emit(
            path,
            lines.len(),
            lines.last().map(|l| l.len()).unwrap_or(0) + 1,
            "le fichier ne se termine pas par une newline",
        );
        count += 1;
    }

    count
}

// ─────────────────────────────────────────────────────────────────────────────
// Traversée des fichiers + suivi des imports
// ─────────────────────────────────────────────────────────────────────────────

fn check_with_imports(
    path:    &Path,
    cfg:     &Config,
    visited: &mut HashSet<PathBuf>,
    total:   &mut usize,
) {
    // Si le fichier n'existe pas, on ignore silencieusement (import de démonstration)
    if !path.exists() { return; }

    let canonical = match path.canonicalize() {
        Ok(p)  => p,
        Err(_) => return,
    };
    if !visited.insert(canonical.clone()) { return; }

    let content = match fs::read_to_string(&canonical) {
        Ok(c)  => c,
        Err(e) => {
            eprintln!("ocaracs: impossible de lire '{}': {}", path.display(), e);
            return;
        }
    };

    // Affichage en chemin relatif au cwd si possible
    let display_path = std::env::current_dir()
        .ok()
        .and_then(|cwd| canonical.strip_prefix(&cwd).ok().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| path.to_path_buf());

    *total += check_file(&display_path, &content, cfg);

    // Suivre les imports utilisateur
    let file_dir = canonical.parent().unwrap_or(Path::new("."));
    for imp in extract_user_imports(&content, file_dir) {
        check_with_imports(&imp, cfg, visited, total);
    }
}

fn check_directory(
    dir:     &Path,
    cfg:     &Config,
    visited: &mut HashSet<PathBuf>,
    total:   &mut usize,
) {
    let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(e)  => e.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(e) => {
            eprintln!("ocaracs: impossible de lire '{}': {}", dir.display(), e);
            return;
        }
    };
    entries.sort();
    for path in entries {
        if path.is_dir() {
            // Ignorer les dossiers cachés et target/
            let name = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if name.starts_with('.') || name == "target" { continue; }
            check_directory(&path, cfg, visited, total);
        } else if path.extension().map(|e| e == "oc").unwrap_or(false) {
            check_with_imports(&path, cfg, visited, total);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Point d'entrée
// ─────────────────────────────────────────────────────────────────────────────

fn print_help() {
    eprintln!("ocaracs — analyseur de style pour Ocara v0.1.0");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  ocaracs <fichier.oc>   Analyser un fichier");
    eprintln!("  ocaracs <dossier>      Analyser tous les .oc d'un dossier");
    eprintln!();
    eprintln!("Configuration:");
    eprintln!("  Fichier .ocaracs à la racine du projet (détecté automatiquement).");
    eprintln!("  Voir docs/tools/ocaracs.md pour la liste des règles et options.");
    eprintln!();
    eprintln!("Codes de sortie:");
    eprintln!("  0  Aucun avertissement");
    eprintln!("  1  Avertissement(s) de style détecté(s)");
    eprintln!("  2  Erreur d'utilisation");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_help();
        std::process::exit(2);
    }

    if args[1] == "--help" || args[1] == "-h" {
        print_help();
        std::process::exit(0);
    }

    let target = PathBuf::from(&args[1]);
    if !target.exists() {
        eprintln!("ocaracs: cible introuvable : {}", target.display());
        std::process::exit(2);
    }

    let project_root = find_project_root(&target);
    let config       = load_config(&project_root);
    let mut visited  = HashSet::new();
    let mut total    = 0usize;

    if target.is_dir() {
        check_directory(&target, &config, &mut visited, &mut total);
    } else {
        check_with_imports(&target, &config, &mut visited, &mut total);
    }

    if total > 0 {
        eprintln!();
        eprintln!("{} avertissement(s) de style trouvé(s).", total);
        std::process::exit(1);
    }
}
