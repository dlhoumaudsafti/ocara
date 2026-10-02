// ─────────────────────────────────────────────────────────────────────────────
// Lecture d'une ligne de source : chaînes, backticks, commentaires
// ─────────────────────────────────────────────────────────────────────────────

use crate::config::{Config, IndentType};

/// État d'une ligne vis-à-vis des chaînes backtick multilignes.
#[derive(Clone, Copy)]
pub struct LineState {
    /// La ligne commence à l'intérieur d'un backtick : son début est du texte.
    pub starts_in_bt: bool,
    /// La ligne se termine à l'intérieur d'un backtick : sa fin est du texte.
    pub ends_in_bt:   bool,
}

impl LineState {
    /// Ligne ENTIÈREMENT à l'intérieur d'un backtick (ni ouverture ni fermeture).
    pub fn inside_bt(self) -> bool {
        self.starts_in_bt && self.ends_in_bt
    }
}

/// Parcourt `line` en suivant chaînes "…"/'…' et backticks (`in_bt` est
/// conservé d'une ligne à l'autre) ; `visit(index, char)` n'est appelé que pour
/// le code. Retourne la position du commentaire `//` éventuel.
pub fn scan_code(line: &str, in_bt: &mut bool, mut visit: impl FnMut(usize, char)) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut chars = line.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if escaped { escaped = false; continue; }
        if c == '\\' && (quote.is_some() || *in_bt) { escaped = true; continue; }
        if *in_bt {
            if c == '`' { *in_bt = false; }
            continue;
        }
        match quote {
            Some(q) => if c == q { quote = None },
            None => match c {
                '"' | '\'' => quote = Some(c),
                '`' => *in_bt = true,
                '/' if chars.peek().is_some_and(|&(_, n)| n == '/') => return Some(i),
                _ => visit(i, c),
            },
        }
    }
    None
}

pub fn line_states(lines: &[&str]) -> Vec<LineState> {
    let mut in_bt = false;
    lines.iter().map(|line| {
        let starts_in_bt = in_bt;
        scan_code(line, &mut in_bt, |_, _| {});
        LineState { starts_in_bt, ends_in_bt: in_bt }
    }).collect()
}

/// Position du commentaire `//` de `line` (hors chaînes et backticks).
pub fn comment_pos(line: &str, state: LineState) -> Option<usize> {
    let mut in_bt = state.starts_in_bt;
    scan_code(line, &mut in_bt, |_, _| {})
}

/// Strippe une visibilité (`public `/`protected `/`private `) en tête de ligne si présente.
pub fn strip_visibility(t: &str) -> &str {
    for vis in ["public ", "protected ", "private "] {
        if let Some(r) = t.strip_prefix(vis) { return r; }
    }
    t
}

/// Premier identifiant de `rest` (avant ` `, `=`, `:`, `(`, `<`…).
pub fn leading_name(rest: &str) -> &str {
    rest.trim().split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("")
}

pub fn leading_ws(line: &str) -> &str {
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// R01/R19/R20 : (tabulations ?, largeur d'un niveau — 0 = non vérifiée),
/// imposés par la configuration, sinon déduits de la première ligne indentée
/// hors backtick. `None` : R01 désactivée ou rien à déduire.
pub fn indent_rule(lines: &[&str], states: &[LineState], cfg: &Config) -> Option<(bool, usize)> {
    if !cfg.indent { return None; }
    let inferred = lines.iter().zip(states)
        .filter(|(l, s)| !s.inside_bt() && !l.is_empty())
        .map(|(l, _)| leading_ws(l))
        .find(|lead| !lead.is_empty());
    let tabs = match cfg.indent_type {
        IndentType::Tab   => Some(true),
        IndentType::Space => Some(false),
        IndentType::Auto  => inferred.map(|u| u.contains('\t')),
    }?;
    let inferred_gap = inferred.filter(|u| !tabs && !u.contains('\t')).map_or(0, str::len);
    Some((tabs, if cfg.indent_gap > 0 { cfg.indent_gap } else { inferred_gap }))
}

#[cfg(test)]
mod tests {
    use super::{comment_pos, line_states};

    #[test]
    fn comments_inside_strings_and_backticks_are_ignored() {
        let lines = ["var u:string = 'http://x' // vrai", "var t:string = `a", "http://b` //c"];
        let states = line_states(&lines);
        assert_eq!(comment_pos(lines[0], states[0]), Some(26));
        assert!(states[1].ends_in_bt && states[2].starts_in_bt && !states[2].ends_in_bt);
        assert_eq!(comment_pos(lines[2], states[2]), Some(10));
    }
}
