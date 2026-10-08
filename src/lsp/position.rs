//! Positions : spans du compilateur (ligne/colonne 1-based, souvent au
//! début de la construction et non du nom) ↔ positions LSP (0-based).

use lsp_types::{Position, Range};

use crate::parsing::token::Span;

/// Plage du nom `name` sur la ligne de `span`, à partir de sa colonne (le nom
/// suit le point d'un `a.b`, la classe d'un `A::b`, le mot-clé d'une
/// déclaration). Le span seul si le nom n'est pas trouvé.
pub fn name_range(text: &str, span: &Span, name: &str) -> Range {
    let line_idx = span.line.saturating_sub(1);
    let start_col = span.col.saturating_sub(1);
    let Some(line) = text.lines().nth(line_idx) else {
        return point(line_idx, start_col);
    };
    let chars: Vec<char> = line.chars().collect();
    let target: Vec<char> = name.chars().collect();
    let found = (start_col..chars.len()).find(|&i| {
        chars[i..].starts_with(&target)
            && (i == 0 || !is_ident(chars[i - 1]))
            && chars.get(i + target.len()).is_none_or(|c| !is_ident(*c))
    });
    match found {
        Some(i) => Range::new(pos(line_idx, i), pos(line_idx, i + target.len())),
        None => point(line_idx, start_col),
    }
}

pub fn point(line: usize, col: usize) -> Range {
    Range::new(pos(line, col), pos(line, col))
}

fn pos(line: usize, col: usize) -> Position {
    Position::new(line as u32, col as u32)
}

pub fn contains(range: &Range, p: &Position) -> bool {
    range.start <= *p && *p <= range.end && range.start != range.end
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
