// ─────────────────────────────────────────────────────────────────────────────
// Configuration `.ocaracs` (section [rules], TOML simplifié)
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::naming::Style;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndentType {
    Auto,
    Space,
    Tab,
}

#[derive(Debug)]
pub struct Config {
    /// R01 — cohérence de l'indentation
    pub indent:            bool,
    /// R02 — pas d'espaces ou tabulations sur les lignes vides
    pub empty_line_ws:     bool,
    /// R03 — espaces autour de '=' dans var / scoped / const
    pub spacing_assign:    bool,
    /// R04 — pas d'espaces ou tabulations en fin de ligne
    pub trailing_ws:       bool,
    /// R05 — longueur max d'une ligne (0 = désactivé)
    pub max_line_length:   usize,
    /// R06 — max lignes vides consécutives (0 = désactivé)
    pub blank_lines_max:   usize,
    /// R07 — classes/structs/interfaces/modules/generics
    pub naming_class:      bool,
    /// R08 — fonctions et méthodes
    pub naming_function:   bool,
    /// R09 — constantes globales et de classe
    pub naming_const:      bool,
    /// R10 — espace après '//' dans les commentaires
    pub comment_spacing:   bool,
    /// R11 — le fichier se termine par une newline
    pub file_ends_newline: bool,
    /// R12 — variables (var/scoped/consumed) et propriétés
    pub naming_variable:   bool,
    /// R13 — const déclarées dans une fonction ou une méthode
    pub naming_const_embed: bool,
    /// R14 — style des const locales (`None` : celui des variables, R16)
    pub const_embed_style: Option<Style>,
    /// R15 — style de R09
    pub const_style:       Style,
    /// R16 — style de R12
    pub var_style:         Style,
    /// R17 — style de R08
    pub function_style:    Style,
    /// R18 — style de R07
    pub class_style:       Style,
    /// R19 — type d'indentation imposé (Auto : déduit de la 1re ligne indentée)
    pub indent_type:       IndentType,
    /// R20 — largeur d'un niveau (0 : déduite de la 1re ligne indentée)
    pub indent_gap:        usize,
    /// R21 — clause `when` collée à sa déclaration (aucune ligne vide)
    pub when_attached:     bool,
    /// R22 — longueur max d'une branche `then`/`else` sur une ligne (0 = désactivé)
    pub cond_branch_max_length: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            indent:             true,
            empty_line_ws:      true,
            spacing_assign:     true,
            trailing_ws:        true,
            max_line_length:    120,
            blank_lines_max:    2,
            naming_class:       true,
            naming_function:    true,
            naming_const:       true,
            comment_spacing:    true,
            file_ends_newline:  true,
            naming_variable:    true,
            naming_const_embed: true,
            const_embed_style:  None,
            const_style:        Style::UpperSnake,
            var_style:          Style::Snake,
            function_style:     Style::Camel,
            class_style:        Style::Pascal,
            indent_type:        IndentType::Auto,
            indent_gap:         0,
            when_attached:      true,
            cond_branch_max_length: 20,
        }
    }
}

impl Config {
    pub fn embed_style(&self) -> Style {
        self.const_embed_style.unwrap_or(self.var_style)
    }
}

const STYLES: &str = "snake_case|camelCase|PascalCase|UPPER_SNAKE_CASE|UPPERCASE";

/// Message d'une valeur invalide (la valeur par défaut est conservée).
fn invalid(key: &str, val: &str, expected: &str) -> String {
    format!("valeur invalide pour {} : '{}' (attendu : {}) — valeur par défaut conservée", key, val, expected)
}

fn set_bool(target: &mut bool, key: &str, val: &str) -> Result<(), String> {
    match val {
        "true"  => *target = true,
        "false" => *target = false,
        _ => return Err(invalid(key, val, "true|false")),
    }
    Ok(())
}

fn set_usize(target: &mut usize, key: &str, val: &str, expected: &str) -> Result<(), String> {
    *target = val.parse().map_err(|_| invalid(key, val, expected))?;
    Ok(())
}

fn set_style(target: &mut Style, key: &str, val: &str) -> Result<(), String> {
    *target = Style::parse(val).ok_or_else(|| invalid(key, val, STYLES))?;
    Ok(())
}

/// Applique `key = val` à `cfg` ; `Err` : clé inconnue ou valeur invalide.
fn set_rule(cfg: &mut Config, key: &str, val: &str) -> Result<(), String> {
    match key {
        "indentation"           => set_bool(&mut cfg.indent, key, val),
        "empty_lines"           => set_bool(&mut cfg.empty_line_ws, key, val),
        "spacing_assign"        => set_bool(&mut cfg.spacing_assign, key, val),
        "trailing_whitespace"   => set_bool(&mut cfg.trailing_ws, key, val),
        "max_line_length"       => set_usize(&mut cfg.max_line_length, key, val, "un entier, 0 = désactivé"),
        "blank_lines_max"       => set_usize(&mut cfg.blank_lines_max, key, val, "un entier, 0 = désactivé"),
        "naming_class"          => set_bool(&mut cfg.naming_class, key, val),
        "naming_function"       => set_bool(&mut cfg.naming_function, key, val),
        "naming_const"          => set_bool(&mut cfg.naming_const, key, val),
        "comment_spacing"       => set_bool(&mut cfg.comment_spacing, key, val),
        "file_ends_newline"     => set_bool(&mut cfg.file_ends_newline, key, val),
        "naming_variable"       => set_bool(&mut cfg.naming_variable, key, val),
        "naming_const_embed"    => set_bool(&mut cfg.naming_const_embed, key, val),
        "when_attached"         => set_bool(&mut cfg.when_attached, key, val),
        "cond_branch_max_length" => set_usize(&mut cfg.cond_branch_max_length, key, val, "un entier, 0 = désactivé"),
        "naming_const_embed_is" => {
            cfg.const_embed_style = Some(Style::parse(val).ok_or_else(|| invalid(key, val, STYLES))?);
            Ok(())
        }
        "naming_const_is"       => set_style(&mut cfg.const_style, key, val),
        "naming_var_is"         => set_style(&mut cfg.var_style, key, val),
        "naming_function_is"    => set_style(&mut cfg.function_style, key, val),
        "naming_class_is"       => set_style(&mut cfg.class_style, key, val),
        "indentation_type"      => {
            cfg.indent_type = match val {
                "auto"  => IndentType::Auto,
                "space" => IndentType::Space,
                "tab"   => IndentType::Tab,
                _ => return Err(invalid(key, val, "auto|space|tab")),
            };
            Ok(())
        }
        "indentation_gap"       => set_usize(&mut cfg.indent_gap, key, val, "un entier, 0 = auto"),
        _ => Err(format!("clé inconnue '{}' (ignorée)", key)),
    }
}

/// Configuration et erreurs (`ligne: message`) du contenu d'un `.ocaracs`.
pub fn parse_config_report(content: &str) -> (Config, Vec<String>) {
    let mut cfg = Config::default();
    let mut errors = Vec::new();
    let mut in_rules = false;
    for (i, raw) in content.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line == "[rules]"        { in_rules = true;  continue; }
        if line.starts_with('[')    { in_rules = false; continue; }
        if !in_rules || line.is_empty() { continue; }
        let result = match line.split_once('=') {
            Some((key, val)) => set_rule(&mut cfg, key.trim(), val.trim()),
            None => Err(format!("ligne sans '=' : '{}' (ignorée)", line)),
        };
        if let Err(e) = result { errors.push(format!("{}: {}", i + 1, e)); }
    }
    (cfg, errors)
}

/// Configuration du contenu d'un `.ocaracs` ; ses erreurs sont signalées.
pub fn parse_config(content: &str) -> Config {
    let (cfg, errors) = parse_config_report(content);
    for e in errors { eprintln!("ocaracs: .ocaracs:{}", e); }
    cfg
}

pub fn load_config(root: &Path) -> Config {
    let path = root.join(".ocaracs");
    match fs::read_to_string(&path) {
        Ok(c)  => parse_config(&c),
        Err(e) => {
            if path.exists() { eprintln!("ocaracs: impossible de lire '{}': {} — configuration par défaut", path.display(), e); }
            Config::default()
        }
    }
}

pub fn find_project_root(path: &Path) -> PathBuf {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        // `x.oc` (sans dossier) : parent vide, à remplacer par le dossier courant.
        path.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
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

#[cfg(test)]
mod tests {
    use super::{parse_config, parse_config_report, IndentType};
    use crate::naming::Style;

    #[test]
    fn defaults_keep_historic_behaviour() {
        let cfg = parse_config("");
        assert_eq!(cfg.const_style, Style::UpperSnake);
        assert_eq!(cfg.embed_style(), Style::Snake);
        assert_eq!(cfg.indent_type, IndentType::Auto);
        assert_eq!(cfg.indent_gap, 0);
    }

    #[test]
    fn embed_style_follows_var_style_unless_set() {
        assert_eq!(parse_config("[rules]\nnaming_var_is = camelCase\n").embed_style(), Style::Camel);
        let cfg = parse_config("[rules]\nnaming_var_is = camelCase\nnaming_const_embed_is = PascalCase\n");
        assert_eq!(cfg.embed_style(), Style::Pascal);
    }

    #[test]
    fn invalid_values_keep_defaults() {
        let cfg = parse_config("[rules]\nnaming_var_is = kebab\nindentation_type = both\n");
        assert_eq!(cfg.var_style, Style::Snake);
        assert_eq!(cfg.indent_type, IndentType::Auto);
    }

    #[test]
    fn errors_are_reported_with_their_line() {
        let src = "[rules]\nindentation = flase\nmax_line_length = 80x\nnaming_clas = true\nblank_lines_max = 3 # commentaire\nnaming_class = false\n";
        let (cfg, errors) = parse_config_report(src);
        assert!(cfg.indent && cfg.max_line_length == 120 && cfg.blank_lines_max == 3 && !cfg.naming_class);
        assert_eq!(errors.len(), 3, "{:?}", errors);
        assert!(errors[0].starts_with("2: valeur invalide pour indentation"));
        assert!(errors[1].starts_with("3: ") && errors[2].starts_with("4: clé inconnue 'naming_clas'"));
    }
}
