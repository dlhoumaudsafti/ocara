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
        }
    }
}

impl Config {
    pub fn embed_style(&self) -> Style {
        self.const_embed_style.unwrap_or(self.var_style)
    }
}

fn invalid(key: &str, val: &str, expected: &str) {
    eprintln!("ocaracs: .ocaracs : valeur invalide pour {} : '{}' (attendu : {}) — valeur par défaut conservée", key, val, expected);
}

const STYLES: &str = "snake_case|camelCase|PascalCase|UPPER_SNAKE_CASE|UPPERCASE";

fn set_style(target: &mut Style, key: &str, val: &str) {
    match Style::parse(val) {
        Some(style) => *target = style,
        None => invalid(key, val, STYLES),
    }
}

pub fn parse_config(content: &str) -> Config {
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
            "indentation"          => cfg.indent             = val == "true",
            "empty_lines"          => cfg.empty_line_ws      = val == "true",
            "spacing_assign"       => cfg.spacing_assign     = val == "true",
            "trailing_whitespace"  => cfg.trailing_ws        = val == "true",
            "max_line_length"      => cfg.max_line_length    = val.parse().unwrap_or(120),
            "blank_lines_max"      => cfg.blank_lines_max    = val.parse().unwrap_or(2),
            "naming_class"         => cfg.naming_class       = val == "true",
            "naming_function"      => cfg.naming_function    = val == "true",
            "naming_const"         => cfg.naming_const       = val == "true",
            "comment_spacing"      => cfg.comment_spacing    = val == "true",
            "file_ends_newline"    => cfg.file_ends_newline  = val == "true",
            "naming_variable"      => cfg.naming_variable    = val == "true",
            "naming_const_embed"   => cfg.naming_const_embed = val == "true",
            "naming_const_embed_is" => match Style::parse(val) {
                Some(style) => cfg.const_embed_style = Some(style),
                None => invalid(key, val, STYLES),
            },
            "naming_const_is"      => set_style(&mut cfg.const_style, key, val),
            "naming_var_is"        => set_style(&mut cfg.var_style, key, val),
            "naming_function_is"   => set_style(&mut cfg.function_style, key, val),
            "naming_class_is"      => set_style(&mut cfg.class_style, key, val),
            "indentation_type"     => match val {
                "auto"  => cfg.indent_type = IndentType::Auto,
                "space" => cfg.indent_type = IndentType::Space,
                "tab"   => cfg.indent_type = IndentType::Tab,
                _ => invalid(key, val, "auto|space|tab"),
            },
            "indentation_gap"      => match val.parse() {
                Ok(n) => cfg.indent_gap = n,
                Err(_) => invalid(key, val, "un entier, 0 = auto"),
            },
            _ => {}
        }
    }
    cfg
}

pub fn load_config(root: &Path) -> Config {
    match fs::read_to_string(root.join(".ocaracs")) {
        Ok(c)  => parse_config(&c),
        Err(_) => Config::default(),
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
    use super::{parse_config, IndentType};
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
}
