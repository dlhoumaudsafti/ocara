//! Liens vers les fichiers (`textDocument/documentLink`) : une chaîne
//! littérale qui désigne un fichier existant — `HTML::renderFile('…')`,
//! `File::read("…")`, `SQLite::open("…")`… — s'ouvre au Ctrl+clic. Le chemin
//! est résolu comme à l'exécution, depuis la racine du projet, puis depuis
//! le dossier du fichier.

use std::path::{Path, PathBuf};

use lsp_types::{DocumentLink, Position, Range};

use super::features::to_url;
use super::position::utf16_col;
use super::workspace::Workspace;

pub fn document_links(ws: &Workspace, path: &Path) -> Vec<DocumentLink> {
    let Some(text) = ws.text(path) else { return Vec::new() };
    let root = ws.project_root(path);
    let dir = path.parent().unwrap_or(&root).to_path_buf();
    string_literals(&text).into_iter()
        .filter_map(|(line, start, end, value)| {
            let target = resolve(&value, &root, &dir)?;
            let text_line = text.lines().nth(line)?;
            Some(DocumentLink {
                range: Range::new(
                    Position::new(line as u32, utf16_col(text_line, start) as u32),
                    Position::new(line as u32, utf16_col(text_line, end) as u32),
                ),
                target: Some(to_url(&target)),
                tooltip: Some(format!("Ouvrir {}", value)),
                data: None,
            })
        })
        .collect()
}

fn resolve(value: &str, root: &Path, dir: &Path) -> Option<PathBuf> {
    if value.is_empty() || value.contains("${") {
        return None;
    }
    let candidate = Path::new(value);
    if candidate.is_absolute() {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    [root, dir].iter().map(|base| base.join(candidate)).find(|p| p.is_file())
}

/// Chaînes `'…'` et `"…"` sur une ligne, hors commentaires et gabarits
/// backtick : (ligne, colonne du premier caractère, colonne après le
/// dernier, contenu), colonnes en caractères.
fn string_literals(text: &str) -> Vec<(usize, usize, usize, String)> {
    let mut out = Vec::new();
    let (mut in_block, mut in_backtick) = (false, false);
    for (line_idx, line) in text.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if in_block {
                if c == '*' && chars.get(i + 1) == Some(&'/') {
                    in_block = false;
                    i += 1;
                }
            } else if in_backtick {
                if c == '\\' {
                    i += 1;
                } else if c == '`' {
                    in_backtick = false;
                }
            } else if c == '/' && chars.get(i + 1) == Some(&'/') {
                break;
            } else if c == '/' && chars.get(i + 1) == Some(&'*') {
                in_block = true;
                i += 1;
            } else if c == '`' {
                in_backtick = true;
            } else if c == '"' || c == '\'' {
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && chars[j] != c {
                    j += if chars[j] == '\\' { 2 } else { 1 };
                }
                if j < chars.len() {
                    out.push((line_idx, start, j, chars[start..j].iter().collect()));
                }
                i = j;
            }
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::string_literals;

    #[test]
    fn finds_quoted_strings_outside_comments_and_templates() {
        let text = "a = f('x/y.html') // \"no\"\n/* \"no\" */ b = `<p \"no\">`\nc = \"é.db\"";
        assert_eq!(string_literals(text), vec![(0, 7, 15, "x/y.html".to_string()), (2, 5, 9, "é.db".to_string())]);
    }
}
