// ─────────────────────────────────────────────────────────────────────────────
// R22 : expression conditionnelle `c then a else b` (docs/EBNF.md §11.3) dont
// une branche dépasse `cond_branch_max_length` caractères sur une seule
// ligne. `--fix` passe `then`/`else` à la ligne quand c'est sûr : au niveau
// de l'instruction (`var x = …`, affectation, `return`), ou dans une
// parenthèse qui ne contient que l'expression — la `)` revient alors seule
// sur sa ligne. Un ternaire d'une branche découpée est réexaminé ensuite.
// ─────────────────────────────────────────────────────────────────────────────

use crate::text::{comment_pos, leading_ws, masked_code, LineState};

/// Mots après lesquels `empty`/`zero`/`null` est un opérande, pas la valeur
/// par défaut d'un `then`.
const BEFORE_OPERAND: [&str; 15] = ["then", "else", "and", "or", "not", "equal", "greater", "smaller", "return", "is", "in", "use", "resolve", "raise", "emit"];

/// Expression `then` entièrement écrite sur une ligne (indices en caractères).
#[derive(Debug, PartialEq)]
pub struct Cond {
    /// Parenthèse/crochet/accolade qui la contient (`None` : niveau de la ligne).
    group:    Option<usize>,
    pub then_at: usize,
    /// `else`, ou la valeur par défaut de la forme courte.
    else_at:  usize,
    default:  bool,
    /// Fin de la branche `else` : fermant du groupe, ou fin du code.
    end:      usize,
    /// Maillon d'une chaîne `x then a else y then b else c` : dans la
    /// branche `else` d'une expression du même groupe.
    chained:  bool,
    /// Sa branche `else` est le maillon suivant, mesuré à part.
    chain_tail: bool,
}

/// Ouvrant d'une ligne : caractère, fermant sur la même ligne, virgule à son
/// niveau, appel (`f(`).
struct Open {
    kind:  char,
    close: Option<usize>,
    comma: bool,
    call:  bool,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Mot qui se termine juste avant l'indice `end` (espaces ignorés).
fn word_before(code: &[char], end: usize) -> (Option<char>, String) {
    let mut i = end;
    while i > 0 && code[i - 1].is_whitespace() { i -= 1; }
    let last = (i > 0).then(|| code[i - 1]);
    let mut start = i;
    while start > 0 && is_word_char(code[start - 1]) { start -= 1; }
    (last, code[start..i].iter().collect())
}

/// `empty`/`zero`/`null` en `at` suit-il une expression complète ?
fn is_default(code: &[char], at: usize, len: usize) -> bool {
    if code.get(at + len).is_some_and(|c| matches!(c, '(' | '.' | '[')) { return false; }
    let (last, word) = word_before(code, at);
    match last {
        Some(c) if is_word_char(c) => !BEFORE_OPERAND.contains(&word.as_str()),
        Some(c) => matches!(c, ')' | ']' | '}' | '"'),
        None => false,
    }
}

/// Code de la ligne (chaînes masquées, commentaire retiré) en caractères.
fn code_of(line: &str, state: LineState) -> Vec<char> {
    masked_code(line, &mut state.starts_in_bt.clone()).chars().collect()
}

/// Ouvrants de `code` (indexés par position) et, pour chaque mot `then`/
/// `else`/valeur par défaut : (position, mot, groupe).
fn scan(code: &[char]) -> (Vec<(usize, Open)>, Vec<(usize, &'static str, Option<usize>)>) {
    let mut opens: Vec<(usize, Open)> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut words = Vec::new();
    let mut i = 0;
    while i < code.len() {
        let c = code[i];
        if is_word_char(c) && (i == 0 || !is_word_char(code[i - 1])) {
            let mut end = i;
            while end < code.len() && is_word_char(code[end]) { end += 1; }
            let word: String = code[i..end].iter().collect();
            let group = stack.last().copied();
            let kind = match word.as_str() {
                "then" => Some("then"),
                "else" => Some("else"),
                "empty" | "zero" | "null" if is_default(code, i, end - i) => Some("default"),
                _ => None,
            };
            if let Some(kind) = kind { words.push((i, kind, group)); }
            i = end;
            continue;
        }
        match c {
            '(' | '[' | '{' => {
                let (last, word) = word_before(code, i);
                let call = c == '(' && last.is_some_and(|l| matches!(l, ')' | ']' | '>')
                    || is_word_char(l) && !BEFORE_OPERAND.contains(&word.as_str()));
                opens.push((i, Open { kind: c, close: None, comma: false, call }));
                stack.push(i);
            }
            ')' | ']' | '}' => {
                if let Some(open) = stack.pop() {
                    if let Some((_, o)) = opens.iter_mut().find(|(at, _)| *at == open) { o.close = Some(i); }
                }
            }
            ',' => {
                if let Some(&open) = stack.last() {
                    if let Some((_, o)) = opens.iter_mut().find(|(at, _)| *at == open) { o.comma = true; }
                }
            }
            _ => {}
        }
        i += 1;
    }
    (opens, words)
}

/// Expressions `then` écrites entièrement sur la ligne ; chaque maillon
/// d'une chaîne est une expression. Un ternaire nu dans une branche `then`
/// est mesuré avec elle.
fn conds(code: &[char]) -> (Vec<(usize, Open)>, Vec<Cond>) {
    let (opens, words) = scan(code);
    let mut found: Vec<Cond> = Vec::new();
    for (k, &(then_at, kind, group)) in words.iter().enumerate() {
        if kind != "then" || found.iter().any(|c| c.group == group && c.then_at < then_at && then_at < c.else_at) { continue; }
        let mut nested = 0usize;
        let matched = words[k + 1..].iter().filter(|w| w.2 == group).find(|&&(_, kind, _)| match kind {
            "then" => { nested += 1; false }
            _ if nested > 0 => { nested -= 1; false }
            _ => true,
        });
        let Some(&(else_at, kind, _)) = matched else { continue };
        let end = group
            .and_then(|g| opens.iter().find(|(at, _)| *at == g).and_then(|(_, o)| o.close))
            .unwrap_or(code.len());
        let chained = found.iter().any(|c| c.group == group && c.else_at < then_at);
        found.push(Cond { group, then_at, else_at, default: kind == "default", end, chained, chain_tail: false });
    }
    let tails: Vec<bool> = found.iter().map(|c| found.iter().any(|o| o.group == c.group && o.then_at > c.else_at)).collect();
    for (cond, tail) in found.iter_mut().zip(tails) { cond.chain_tail = tail; }
    (opens, found)
}

fn text(chars: &[char], from: usize, to: usize) -> String {
    chars[from..to].iter().collect::<String>().trim().to_string()
}

/// Branches (`then`, `else` — vide pour une valeur par défaut) de `cond`.
fn branches(line: &[char], cond: &Cond) -> (String, String) {
    let then_branch = text(line, cond.then_at + 4, cond.else_at);
    let else_branch = if cond.default || cond.chain_tail { String::new() } else { text(line, cond.else_at + 4, cond.end) };
    (then_branch, else_branch)
}

/// R22 : expressions de la ligne dont une branche dépasse `max` caractères,
/// avec la longueur de la plus longue branche.
pub fn long_conds(line: &str, state: LineState, max: usize) -> Vec<(Cond, usize)> {
    if max == 0 || state.starts_in_bt { return Vec::new(); }
    let chars: Vec<char> = line.chars().collect();
    conds(&code_of(line, state)).1.into_iter().filter_map(|cond| {
        let (a, b) = branches(&chars, &cond);
        let longest = a.chars().count().max(b.chars().count());
        (longest > max).then_some((cond, longest))
    }).collect()
}

/// Affectation (`=` seul, ni `==`, `=>`, `<=`…) ou `return` en tête.
fn is_statement_value(before: &[char]) -> bool {
    let head: String = before.iter().collect();
    head.trim_start().starts_with("return ") || (0..before.len()).any(|j| {
        let prev = if j > 0 { before[j - 1] } else { ' ' };
        let next = before.get(j + 1).copied().unwrap_or(' ');
        before[j] == '=' && !matches!(next, '=' | '>') && !matches!(prev, '=' | '!' | '<' | '>')
    })
}

/// `--fix` R22 : la ligne découpée (`unit` : un niveau d'indentation), ou
/// `None` si rien n'est à découper sans risque.
pub fn split(line: &str, state: LineState, max: usize, unit: &str) -> Option<Vec<String>> {
    if state.starts_in_bt || state.ends_in_bt || comment_pos(line, state).is_some() { return None; }
    let code = code_of(line, state);
    let chars: Vec<char> = line.chars().collect();
    let (opens, _) = conds(&code);
    let lead = leading_ws(line);
    let inner = format!("{}{}", lead, unit);
    long_conds(line, state, max).into_iter().filter(|(cond, _)| !cond.chained && !cond.chain_tail).find_map(|(cond, _)| {
        let (then_branch, else_branch) = branches(&chars, &cond);
        let else_line = if cond.default { text(&chars, cond.else_at, chars.len().min(cond.end)) } else { format!("else {}", else_branch) };
        let branch_lines = [format!("{}then {}", inner, then_branch), format!("{}{}", inner, else_line)];
        match cond.group {
            None if is_statement_value(&code[..cond.then_at]) => {
                let head = chars[..cond.then_at].iter().collect::<String>().trim_end().to_string();
                Some([vec![head], branch_lines.to_vec()].concat())
            }
            Some(open) => {
                let (_, o) = opens.iter().find(|(at, _)| *at == open)?;
                let close = o.close?;
                if o.kind != '(' || o.call || o.comma { return None; }
                let head = format!("{}{}", chars[..=open].iter().collect::<String>(), text(&chars, open + 1, cond.then_at));
                let tail = format!("{}{}", lead, chars[close..].iter().collect::<String>());
                Some([vec![head], branch_lines.to_vec(), vec![tail]].concat())
            }
            None => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{long_conds, split};
    use crate::text::LineState;

    fn long(line: &str) -> Vec<usize> {
        long_conds(line, LineState::default(), 20).into_iter().map(|(_, n)| n).collect()
    }

    fn split20(line: &str) -> Option<Vec<String>> {
        split(line, LineState::default(), 20, "    ")
    }

    #[test]
    fn measures_each_branch_without_its_keyword() {
        assert!(long("var s:string = age greater 17 then \"majeur\" else \"mineur\"").is_empty());
        assert_eq!(long("var f:F = k greater 2 then nameless(x:int): int { return x + k } else g"), vec![37]);
        assert_eq!(long("var s:string = ok then user.name.toUpperCase() empty"), vec![23]);
    }

    #[test]
    fn measures_each_link_of_a_chain() {
        assert!(long("return n greater 0 then \"positif\" else n equal 0 then \"nul\" else \"négatif\"").is_empty());
        assert_eq!(long("return a then 1 else b then aVeryLongValueForTheLink else 3"), vec![24]);
        assert!(split20("    return a then 1 else b then aVeryLongValueForTheLink else 3").is_none());
    }

    #[test]
    fn ignores_strings_comments_and_identifiers_named_like_defaults() {
        assert!(long("var s:string = \"a then bbbbbbbbbbbbbbbbbbbbbbbbbbb else c\"").is_empty());
        assert!(long("var s:int = a // c then bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb else c").is_empty());
        assert_eq!(long("var z:int = zero equal 0 then zero zero"), Vec::<usize>::new());
        assert_eq!(long("var z:int = c then zero.aVeryLongMethodName() else zero"), vec![26]);
    }

    #[test]
    fn splits_a_statement_value() {
        assert_eq!(split20("    var s:string = ok then user.name.toUpperCase() empty"), Some(vec![
            "    var s:string = ok".to_string(),
            "        then user.name.toUpperCase()".to_string(),
            "        empty".to_string(),
        ]));
    }

    #[test]
    fn splits_a_parenthesized_expression_and_keeps_what_follows() {
        assert_eq!(split20("    var p:int = (age greater 10 then (age greater 30 then 3 else 2) else 1) * 100"), Some(vec![
            "    var p:int = (age greater 10".to_string(),
            "        then (age greater 30 then 3 else 2)".to_string(),
            "        else 1".to_string(),
            "    ) * 100".to_string(),
        ]));
    }

    #[test]
    fn leaves_unsafe_places_to_the_warning() {
        assert!(split20("    f(x, ok then aVeryLongArgumentValue else 0)").is_none());
        assert!(split20("    if ok then aVeryLongConditionValue else b {").is_none());
        assert!(split20("    var s:int = ok then aVeryLongBranchValue else 0 // note").is_none());
    }
}
