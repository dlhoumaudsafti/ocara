//! Repérage syntaxique autour du curseur : appel en cours de frappe et texte
//! « patché » (nom en cours de frappe remplacé par le marqueur de
//! complétion, parenthèses non fermées refermées) que l'analyse sait parser.

use lsp_types::Position;

use crate::sema::index::COMPLETION_MARKER;

/// Délimiteur ouvert (`(`, `[`, `{`) avec les virgules de premier niveau.
struct Frame {
    open: char,
    start: usize,
    commas: Vec<usize>,
}

pub struct CallSite {
    /// Nom appelé et position de son premier caractère.
    pub callee: String,
    pub callee_pos: Position,
    /// Appel de méthode sur une valeur (`a.m(`), pas statique (`A::m(`).
    pub on_instance: bool,
    pub arg_index: usize,
    pub positional_count: usize,
    /// Noms des arguments nommés déjà écrits avant l'argument courant.
    pub used_names: Vec<String>,
    /// Argument courant déjà nommé (`f(a: |`).
    pub current_name: Option<String>,
}

pub fn offset(text: &str, pos: &Position) -> usize {
    let mut offset = 0;
    for (i, line) in text.split('\n').enumerate() {
        if i == pos.line as usize {
            return offset + (pos.character as usize).min(line.chars().count());
        }
        offset += line.chars().count() + 1;
    }
    offset
}

fn position(chars: &[char], index: usize) -> Position {
    let before = &chars[..index];
    let line = before.iter().filter(|c| **c == '\n').count();
    let col = before.iter().rev().take_while(|c| **c != '\n').count();
    Position::new(line as u32, col as u32)
}

/// Délimiteurs ouverts avant `end` (chaînes et commentaires ignorés).
fn frames_until(chars: &[char], end: usize) -> Vec<Frame> {
    let mut stack: Vec<Frame> = Vec::new();
    let mut i = 0;
    while i < end {
        let c = chars[i];
        match c {
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < end && chars[i] != '\n' { i += 1; }
                continue;
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i + 1 < end && !(chars[i] == '*' && chars[i + 1] == '/') { i += 1; }
                i += 2;
                continue;
            }
            '"' | '\'' | '`' => {
                i += 1;
                while i < end && chars[i] != c {
                    if chars[i] == '\\' { i += 1; }
                    i += 1;
                }
            }
            '(' | '[' | '{' => stack.push(Frame { open: c, start: i, commas: Vec::new() }),
            ')' | ']' | '}' => { stack.pop(); }
            ',' => if let Some(top) = stack.last_mut() { top.commas.push(i); },
            _ => {}
        }
        i += 1;
    }
    stack
}

fn closer(open: char) -> char {
    match open { '(' => ')', '[' => ']', _ => '}' }
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Appel dont les parenthèses entourent le curseur.
pub fn call_site(text: &str, pos: &Position) -> Option<CallSite> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = offset(text, pos);
    let frame = frames_until(&chars, cursor).pop().filter(|f| f.open == '(')?;

    let name_end = frame.start;
    let name_start = (0..name_end).rev().take_while(|&i| is_ident(chars[i])).last()?;
    let callee: String = chars[name_start..name_end].iter().collect();
    let on_instance = name_start > 0 && chars[name_start - 1] == '.';

    let mut bounds = vec![frame.start + 1];
    bounds.extend(frame.commas.iter().map(|c| c + 1));
    let segment = |from: usize, to: usize| -> String { chars[from..to].iter().collect() };
    let mut used_names = Vec::new();
    let mut positional_count = 0;
    for w in bounds.windows(2) {
        match arg_name(&segment(w[0], w[1] - 1)) {
            Some(name) => used_names.push(name),
            None => positional_count += 1,
        }
    }
    let current_name = arg_name(&segment(*bounds.last()?, cursor));
    Some(CallSite {
        callee,
        callee_pos: position(&chars, name_start),
        on_instance,
        arg_index: frame.commas.len(),
        positional_count,
        used_names,
        current_name,
    })
}

/// `nom:` en tête d'un argument (pas `A::b`).
fn arg_name(arg: &str) -> Option<String> {
    let trimmed = arg.trim_start();
    let name: String = trimmed.chars().take_while(|c| is_ident(*c)).collect();
    let rest = trimmed[name.len()..].trim_start();
    (!name.is_empty() && rest.starts_with(':') && !rest.starts_with("::")).then_some(name)
}

/// Texte où l'identifiant sous le curseur (avant et après lui) est remplacé
/// par le marqueur, et où les délimiteurs ouverts avant lui et jamais refermés
/// ensuite sont refermés en fin de ligne.
pub fn patched_text(text: &str, pos: &Position) -> String {
    let chars: Vec<char> = text.chars().collect();
    let cursor = offset(text, pos);
    let start = (0..cursor).rev().take_while(|&i| is_ident(chars[i])).last().unwrap_or(cursor);

    let end = (cursor..chars.len()).find(|&i| !is_ident(chars[i])).unwrap_or(chars.len());
    let mut open = frames_until(&chars, start);
    let mut depth = Vec::new();
    for &c in &chars[end..] {
        match c {
            '(' | '[' | '{' => depth.push(c),
            ')' | ']' | '}' => if depth.pop().is_none() { open.pop(); },
            _ => {}
        }
    }
    let closers: String = open.iter().rev().map(|f| closer(f.open)).collect();

    let line_end = chars[end..].iter().position(|c| *c == '\n').map_or(chars.len(), |i| end + i);
    let mut out: String = chars[..start].iter().collect();
    out.push_str(COMPLETION_MARKER);
    out.extend(&chars[end..line_end]);
    out.push_str(&closers);
    out.extend(&chars[line_end..]);
    out
}

/// Vrai si `pos` est dans du code, pas dans une chaîne ni un commentaire.
pub fn is_code_at(text: &str, pos: &Position) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let end = offset(text, pos);
    let mut i = 0;
    while i < end {
        match chars[i] {
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    if i >= end { return false; }
                    i += 1;
                }
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    if i >= end { return false; }
                    i += 1;
                }
                i += 2;
                continue;
            }
            q @ ('"' | '\'' | '`') => {
                i += 1;
                while i < chars.len() && chars[i] != q {
                    if i >= end { return false; }
                    if chars[i] == '\\' { i += 1; }
                    i += 1;
                }
                if i >= end { return false; }
            }
            _ => {}
        }
        i += 1;
    }
    true
}
