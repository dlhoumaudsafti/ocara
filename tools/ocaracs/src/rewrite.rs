// ─────────────────────────────────────────────────────────────────────────────
// Réécriture des identifiants d'un fichier (renommage) : chaque identifiant du
// code — y compris dans les `${…}` des backticks, jamais dans une chaîne ni un
// commentaire — est proposé à un visiteur avec sa position (`Access`), qui
// peut le remplacer. Les fins de ligne sont recopiées telles quelles.
// ─────────────────────────────────────────────────────────────────────────────

use crate::members::{receiver_before, MemberAccess};

/// Position d'un identifiant dans le code.
#[derive(Debug, PartialEq)]
pub enum Access {
    /// Identifiant nu (déclaration, appel de fonction, variable…).
    Bare,
    /// Après `.`/`::`.
    Member(MemberAccess),
    /// Segment d'import qui désigne un fichier (`import lib.nom`, chemin
    /// `from "…/nom"`) : renommé seulement si le fichier l'est aussi.
    ImportPath,
}

pub type Visitor<'a> = dyn FnMut(&str, &Access) -> Option<String> + 'a;

enum Mode {
    Code,
    Quote(char),
    Backtick,
    /// `${` … `}` dans un backtick : code, avec profondeur d'accolades.
    Interp(usize),
}

/// Recopie `content` en passant chaque identifiant du code à `visit`. Les
/// lignes `import` sont traitées à part (`rewrite_import`),
/// `namespace`/`runtime` recopiées.
pub fn rewrite(content: &str, visit: &mut Visitor) -> String {
    let mut out = String::with_capacity(content.len());
    let mut modes = vec![Mode::Code];
    for (line_no, line) in content.split_inclusive('\n').enumerate() {
        let t = line.trim_start();
        if matches!(modes.last(), Some(Mode::Code)) && modes.len() == 1 {
            if t.starts_with("import ") {
                out.push_str(&rewrite_import(line, visit));
                continue;
            }
            if t.starts_with("namespace ") || t.starts_with("runtime ") {
                out.push_str(line);
                continue;
            }
        }
        rewrite_line(line, line_no, &mut modes, &mut out, visit);
    }
    out
}

/// Position d'un identifiant qui commence en `start` de la ligne `line_no`
/// (0-based) : après `.`/`::` ou nu.
fn access_at(chars: &[char], start: usize, line_no: usize) -> Access {
    let mut k = start;
    while k > 0 && chars[k - 1] == ' ' { k -= 1; }
    let path = k >= 2 && chars[k - 1] == ':' && chars[k - 2] == ':';
    if !path && !(k >= 1 && chars[k - 1] == '.') { return Access::Bare; }
    let op_start = if path { k - 2 } else { k - 1 };
    Access::Member(MemberAccess { receiver: receiver_before(chars, op_start), path, line: line_no })
}

fn rewrite_line(line: &str, line_no: usize, modes: &mut Vec<Mode>, out: &mut String, visit: &mut Visitor) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match modes.last_mut() {
            Some(Mode::Quote(q)) => {
                let q = *q;
                out.push(c);
                if c == '\\' { if let Some(n) = next { out.push(n); i += 1; } }
                else if c == q { modes.pop(); }
            }
            Some(Mode::Backtick) => {
                out.push(c);
                if c == '\\' { if let Some(n) = next { out.push(n); i += 1; } }
                else if c == '`' { modes.pop(); }
                else if c == '$' && next == Some('{') { out.push('{'); i += 1; modes.push(Mode::Interp(0)); }
            }
            Some(Mode::Code) | Some(Mode::Interp(_)) => {
                if c == '/' && next == Some('/') {
                    out.extend(&chars[i..]);
                    return;
                }
                if c.is_alphabetic() || c == '_' {
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') { i += 1; }
                    let name: String = chars[start..i].iter().collect();
                    let digit_prefixed = start > 0 && chars[start - 1].is_ascii_digit();
                    match visit(&name, &access_at(&chars, start, line_no)).filter(|_| !digit_prefixed) {
                        Some(new) => out.push_str(&new),
                        None => out.push_str(&name),
                    }
                    continue;
                }
                out.push(c);
                match (c, modes.last_mut()) {
                    ('"' | '\'', _) => modes.push(Mode::Quote(c)),
                    ('`', _) => modes.push(Mode::Backtick),
                    ('{', Some(Mode::Interp(depth))) => *depth += 1,
                    ('}', Some(Mode::Interp(0))) => { modes.pop(); }
                    ('}', Some(Mode::Interp(depth))) => *depth -= 1,
                    _ => {}
                }
            }
            None => out.push(c),
        }
        i += 1;
    }
}

/// `import a.b.nom (as Alias)?` : le dernier segment désigne à la fois le
/// fichier `a/b/nom.oc` et le symbole `nom` ; il n'est renommé que si le
/// fichier l'est (`Access::ImportPath`), sinon l'import ne résoudrait plus.
/// `import Nom from "chemin/fichier" (as Alias)?` : le symbole et l'alias
/// sont renommés, le dernier composant du chemin seulement si le fichier
/// l'est. Un commentaire de fin est recopié tel quel.
fn rewrite_import(line: &str, visit: &mut Visitor) -> String {
    let lead = &line[..line.len() - line.trim_start().len()];
    let ending = &line[line.trim_end().len()..];
    let full = line.trim();
    let (body, comment) = match crate::text::comment_pos(full, crate::text::LineState::default()) {
        Some(c) => (full[..c].trim_end(), &full[full[..c].trim_end().len()..]),
        None => (full, ""),
    };
    let (main, alias) = match body.split_once(" as ") {
        Some((m, a)) => (m, Some(a.trim())),
        None => (body, None),
    };
    let main = main.strip_prefix("import ").unwrap_or(main).trim();
    let mut rename = |name: &str, access: Access| visit(name, &access).unwrap_or_else(|| name.to_string());
    let rewritten = if let Some((target, path)) = main.split_once(" from ") {
        let inner = path.trim().trim_matches('"');
        let (dir, file) = inner.rsplit_once('/').map_or(("", inner), |(d, f)| (d, f));
        let (stem, ext) = file.strip_suffix(".oc").map_or((file, ""), |s| (s, ".oc"));
        let stem = rename(stem, Access::ImportPath);
        let new_path = if dir.is_empty() { format!("{}{}", stem, ext) } else { format!("{}/{}{}", dir, stem, ext) };
        let target = if target == "*" { target.to_string() } else { rename(target, Access::Bare) };
        format!("{} from \"{}\"", target, new_path)
    } else if main.starts_with("ocara.") {
        main.to_string()
    } else {
        match main.rsplit_once('.') {
            Some((dirs, last)) => format!("{}.{}", dirs, rename(last, Access::ImportPath)),
            None => rename(main, Access::ImportPath),
        }
    };
    let alias = alias.map(|a| format!(" as {}", rename(a, Access::Bare))).unwrap_or_default();
    format!("{}import {}{}{}{}", lead, rewritten, alias, comment, ending)
}
