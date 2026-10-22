// ─────────────────────────────────────────────────────────────────────────────
// Analyse lexicale minimale d'un source Ocara
//
// Retire commentaires `//` et contenu des chaînes ("…", '…', `…`) pour que
// le comptage d'accolades et la reconnaissance des déclarations ne soient
// pas trompés par du texte. Les sauts de ligne sont conservés : la ligne N
// du résultat est la ligne N du source.
// ─────────────────────────────────────────────────────────────────────────────

/// Source sans commentaires ni contenu de chaînes, ligne par ligne.
pub fn code_lines(source: &str) -> Vec<String> {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '/' if chars.peek() == Some(&'/') => {
                while chars.peek().is_some_and(|&c| c != '\n') { chars.next(); }
            }
            '"' | '\'' | '`' => {
                out.push(ch);
                skip_string(&mut chars, ch, &mut out);
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out.lines().map(str::to_string).collect()
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// Avance jusqu'au délimiteur fermant `quote` (échappements `\` sautés) ;
/// seuls les sauts de ligne du contenu sont recopiés. Dans un gabarit,
/// `${ … }` est du code, qui peut contenir d'autres chaînes et gabarits.
fn skip_string(chars: &mut Chars, quote: char, out: &mut String) {
    while let Some(c) = chars.next() {
        match c {
            '\\' => { chars.next(); }
            '\n' => out.push('\n'),
            '$' if quote == '`' && chars.peek() == Some(&'{') => {
                chars.next();
                skip_interpolation(chars, out);
            }
            c if c == quote => return,
            _ => {}
        }
    }
}

/// Avance jusqu'à l'accolade qui ferme `${`.
fn skip_interpolation(chars: &mut Chars, out: &mut String) {
    let mut depth = 1;
    while let Some(c) = chars.next() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
            '"' | '\'' | '`' => skip_string(chars, c, out),
            '\n' => out.push('\n'),
            _ => {}
        }
    }
}

/// Mots d'une ligne de code : séparés par les espaces et la ponctuation
/// de déclaration (`(`, `{`, `<`, `:`), ce qui tolère `public  method`,
/// tabulations et `fooTest(`.
pub fn words(line: &str) -> Vec<&str> {
    line.split(|c: char| c.is_whitespace() || "({<:".contains(c))
        .filter(|w| !w.is_empty())
        .collect()
}

/// Variation de profondeur d'accolades sur une ligne de code.
pub fn brace_delta(line: &str) -> i32 {
    line.chars().map(|c| match c { '{' => 1, '}' => -1, _ => 0 }).sum()
}
