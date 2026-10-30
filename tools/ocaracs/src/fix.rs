// ─────────────────────────────────────────────────────────────────────────────
// `--fix` : mise en forme d'un fichier (R01, R02, R03, R04, R06, R10, R11, R21, R22).
// Le contenu des chaînes backtick multilignes n'est jamais modifié. R05
// (ligne trop longue) n'est pas corrigeable automatiquement.
// ─────────────────────────────────────────────────────────────────────────────

use crate::check::{decl_assign_pos, detached_when_blanks};
use crate::cond::split;
use crate::config::Config;
use crate::text::{comment_pos, indent_rule, leading_ws, line_states, masked_code, scan_code, LineState};

/// Découpe `content` en lignes (sans fin de ligne) et fins de ligne
/// (`"\r\n"`, `"\n"` ou `""` pour une dernière ligne sans newline) : chaque
/// ligne garde la sienne, le contenu des backticks n'est donc jamais modifié.
fn split_lines(content: &str) -> (Vec<&str>, Vec<&str>) {
    content.split_inclusive('\n').map(|piece| {
        let body = piece.strip_suffix('\n').map_or(piece, |b| b.strip_suffix('\r').unwrap_or(b));
        (body, &piece[body.len()..])
    }).unzip()
}

/// Tours de R22 au plus : chacun découpe un niveau de ternaires imbriqués.
const MAX_SPLIT_PASSES: usize = 32;

/// R22 : branches `then`/`else` trop longues passées à la ligne, jusqu'à ce
/// qu'il n'y ait plus rien à découper — un ternaire d'une branche découpée
/// est réexaminé au tour suivant.
fn split_long_conds(content: &str, cfg: &Config) -> String {
    let mut content = content.to_string();
    if cfg.cond_branch_max_length == 0 { return content; }
    let eol = if content.contains("\r\n") { "\r\n" } else { "\n" };
    for _ in 0..MAX_SPLIT_PASSES {
        let next = {
            let (lines, endings) = split_lines(&content);
            let states = line_states(&lines);
            let unit = indent_unit(&lines, &states, cfg);
            let mut changed = false;
            let next: String = lines.iter().zip(&endings).zip(&states).map(|((line, end), state)| {
                match split(line, *state, cfg.cond_branch_max_length, &unit) {
                    Some(parts) => { changed = true; format!("{}{}", parts.join(eol), end) }
                    None => format!("{}{}", line, end),
                }
            }).collect();
            changed.then_some(next)
        };
        match next {
            Some(next) => content = next,
            None => break,
        }
    }
    content
}

pub fn fix_layout(content: &str, cfg: &Config) -> String {
    let content = split_long_conds(content, cfg);
    let content = content.as_str();
    let (lines, endings) = split_lines(content);
    let states = line_states(&lines);
    let mut out: Vec<String> = lines.iter().zip(&states)
        .map(|(line, state)| fix_line(line, *state, cfg))
        .collect();
    if cfg.indent {
        let as_str: Vec<&str> = out.iter().map(String::as_str).collect();
        let unit = indent_unit(&lines, &states, cfg);
        out = reindent(&as_str, &states, &unit);
    }
    // R21 : lignes vides entre une clause `when` et sa déclaration retirées.
    let detached: Vec<usize> = if cfg.when_attached {
        detached_when_blanks(&lines, &states).into_iter().map(|(_, i)| i).collect()
    } else {
        Vec::new()
    };
    let kept = |i: &usize| !detached.contains(i);
    let mut out: Vec<(String, &str)> = out.into_iter().zip(endings).enumerate()
        .filter(|(i, _)| kept(i)).map(|(_, l)| l).collect();
    if cfg.blank_lines_max > 0 {
        let states: Vec<LineState> = states.iter().enumerate().filter(|(i, _)| kept(i)).map(|(_, s)| *s).collect();
        out = collapse_blank_lines(out, &states, cfg.blank_lines_max);
    }
    // R11 : newline finale dans le style du fichier.
    let eol = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let mut result: String = out.iter().map(|(l, e)| format!("{}{}", l, e)).collect();
    if !result.is_empty() && out.last().is_some_and(|(_, e)| e.is_empty()) && cfg.file_ends_newline {
        result.push_str(eol);
    }
    result
}

/// R03, R10, R04, R02 sur une ligne dont le début est du code.
fn fix_line(line: &str, state: LineState, cfg: &Config) -> String {
    if state.starts_in_bt {
        // Fin de chaîne sur cette ligne : ce qui suit le backtick est du code.
        let trim = cfg.trailing_ws && !state.ends_in_bt;
        return if trim { line.trim_end().to_string() } else { line.to_string() };
    }
    let mut line = line.to_string();
    if cfg.spacing_assign {
        if let Some(j) = decl_assign_pos(&line) {
            let after_space = line[j + 1..].starts_with([' ', '\t']) || j + 1 == line.len();
            let before_space = j > 0 && line[..j].ends_with([' ', '\t']);
            line = format!("{}{}={}{}", &line[..j], if before_space { "" } else { " " }, if after_space { "" } else { " " }, &line[j + 1..]);
        }
    }
    if cfg.comment_spacing {
        if let Some(pos) = comment_pos(&line, state) {
            let after = &line[pos + 2..];
            if !after.is_empty() && !after.starts_with(' ') && !after.starts_with('/') {
                line.insert(pos + 2, ' ');
            }
        }
    }
    if (cfg.trailing_ws || cfg.empty_line_ws) && !state.ends_in_bt {
        let blank = line.trim().is_empty();
        if (blank && cfg.empty_line_ws) || (!blank && cfg.trailing_ws) {
            line.truncate(line.trim_end().len());
        }
    }
    line
}

/// Texte d'un niveau d'indentation (R19/R20, sinon déduit, sinon 4 espaces).
fn indent_unit(lines: &[&str], states: &[LineState], cfg: &Config) -> String {
    let (tabs, gap) = indent_rule(lines, states, cfg).unwrap_or((false, 0));
    match (tabs, gap) {
        (true, 0)  => "\t".to_string(),
        (true, n)  => "\t".repeat(n),
        (false, 0) => "    ".to_string(),
        (false, n) => " ".repeat(n),
    }
}

const CONTINUATION_STARTS: [&str; 9] = [".", "+ ", "- ", "* ", "/ ", "&& ", "|| ", "and ", "or "];
const CONTINUATION_ENDS: [&str; 7] = ["=", "+", "-", "*", "/", "&&", "||"];

/// Ligne d'une expression `then` au niveau d'une instruction : `then …`,
/// `else …` (pas le `else {` d'un `if`), ou la valeur par défaut seule.
/// Entre parenthèses, la parenthèse donne déjà le niveau.
fn is_cond_line(trimmed: &str) -> bool {
    let word = trimmed.split_whitespace().next().unwrap_or("");
    match word {
        "then" => true,
        "else" => !trimmed.trim_end().ends_with('{'),
        "empty" | "zero" | "null" => trimmed.split("//").next().is_some_and(|code| code.trim() == word),
        _ => false,
    }
}

/// Ligne prolongeant l'instruction précédente (`.chain()`, opérateur en
/// tête, ligne précédente terminée par un opérateur, ou branche d'un `then`
/// hors parenthèses) : un niveau de plus.
fn is_continuation(trimmed: &str, prev_code: Option<&str>, in_parens: bool) -> bool {
    let starts = CONTINUATION_STARTS.iter().any(|p| trimmed.starts_with(p)) && !trimmed.starts_with("..")
        || !in_parens && is_cond_line(trimmed);
    let ends = prev_code.is_some_and(|p| {
        CONTINUATION_ENDS.iter().any(|e| p.ends_with(e))
            && !["++", "--", "==", ".*"].iter().any(|e| p.ends_with(e))
    });
    starts || ends
}

/// R01 : niveau = nombre de LIGNES portant une accolade/parenthèse/crochet
/// encore ouvert (plusieurs ouvrants sur une même ligne, comme
/// `route("/", "GET", nameless(...) {`, ne comptent qu'un niveau) ; les
/// fermants en tête de ligne sont retirés avant de mesurer.
fn reindent(lines: &[&str], states: &[LineState], unit: &str) -> Vec<String> {
    // Ligne de chaque ouvrant encore ouvert, et s'il s'agit d'une accolade.
    let mut stack: Vec<(usize, bool)> = Vec::new();
    let mut in_bt = false;
    let mut prev_code: Option<String> = None;
    lines.iter().zip(states).enumerate().map(|(i, (line, state))| {
        let code_line = !state.starts_in_bt;
        let trimmed = line.trim_start();
        let lead_closers = if code_line {
            trimmed.chars().take_while(|c| matches!(c, '}' | ')' | ']')).count()
        } else { 0 };
        for _ in 0..lead_closers { stack.pop(); }
        let mut level = stack.windows(2).filter(|w| w[0].0 != w[1].0).count() + usize::from(!stack.is_empty());
        let in_parens = stack.last().is_some_and(|&(_, brace)| !brace);
        // Code de la ligne, le texte des chaînes masqué : un `*` final dans
        // un SQL backtick n'est pas un opérateur.
        let code = masked_code(line, &mut in_bt.clone());
        scan_code(line, &mut in_bt, |pos, c| {
            if code_line && pos < leading_ws(line).len() + lead_closers { return; }
            match c {
                '{' | '(' | '[' => stack.push((i, c == '{')),
                '}' | ')' | ']' => { stack.pop(); }
                _ => {}
            }
        });
        let previous = prev_code.clone();
        if !code.trim().is_empty() { prev_code = Some(code.trim().to_string()); }
        if !code_line || trimmed.is_empty() { return line.to_string(); }
        if lead_closers == 0 && is_continuation(trimmed, previous.as_deref(), in_parens) { level += 1; }
        format!("{}{}", unit.repeat(level), trimmed)
    }).collect()
}

/// R06 : au plus `max` lignes vides consécutives (hors backticks).
fn collapse_blank_lines<'a>(lines: Vec<(String, &'a str)>, states: &[LineState], max: usize) -> Vec<(String, &'a str)> {
    let mut blanks = 0usize;
    lines.into_iter().zip(states).filter_map(|(line, state)| {
        if line.0.trim().is_empty() && !state.inside_bt() {
            blanks += 1;
            (blanks <= max).then_some(line)
        } else {
            blanks = 0;
            Some(line)
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::fix_layout;
    use crate::config::parse_config;

    fn fix(src: &str) -> String {
        fix_layout(src, &parse_config(""))
    }

    #[test]
    fn reindents_by_brace_depth() {
        let src = "function main(): int {\n    var x:int=1\n      if x > 0 {\n   x = 2\n }\n  return 0\n}";
        assert_eq!(fix(src), "function main(): int {\n    var x:int = 1\n    if x > 0 {\n        x = 2\n    }\n    return 0\n}\n");
    }

    #[test]
    fn several_openers_on_one_line_count_as_one_level() {
        let src = "server.route(\"/\", \"GET\", nameless(req:HTTPServerRequest): int {\nreq.respond(200, \"Hello\")\nreturn 0\n})\n";
        assert_eq!(fix(src), "server.route(\"/\", \"GET\", nameless(req:HTTPServerRequest): int {\n    req.respond(200, \"Hello\")\n    return 0\n})\n");
    }

    #[test]
    fn backtick_content_is_untouched() {
        let src = "function f(): string {\nvar h:string = `<div>   \n   {  }   \n</div>`  \nreturn h\n}\n";
        assert_eq!(fix(src), "function f(): string {\n    var h:string = `<div>   \n   {  }   \n</div>`\n    return h\n}\n");
    }

    #[test]
    fn when_clause_is_glued_to_its_declaration() {
        let src = "when System::OS is 'linux'\n\n\nfunction f(): int { return 1 }\n\nwhen default\nfunction f(): int { return 2 }\n";
        assert_eq!(fix(src), "when System::OS is 'linux'\nfunction f(): int { return 1 }\n\nwhen default\nfunction f(): int { return 2 }\n");
    }

    #[test]
    fn line_endings_are_preserved() {
        let src = "function f(): string {\r\nvar s:string = `a\r\nb`\r\nreturn s\r\n}";
        assert_eq!(fix(src), "function f(): string {\r\n    var s:string = `a\r\nb`\r\n    return s\r\n}\r\n");
        assert_eq!(fix("var x:int = 1\r\n\r\n\r\n\r\nvar y:int = 2\n"), "var x:int = 1\r\n\r\n\r\nvar y:int = 2\n");
    }

    #[test]
    fn string_or_wildcard_ending_with_star_is_not_a_continuation() {
        let src = "import ocara.*\nimport lib.A\nvar q:string = `SELECT *\nFROM t`\nvar n:int = 1\nvar m:string = \"a *\"\nvar k:int = 2\n";
        assert_eq!(fix(src), src);
    }

    #[test]
    fn conditional_branches_are_continuations_outside_parentheses() {
        let src = "function f(): void {\nvar t:string = k equal 2\nthen \"a\"\nempty\nvar u:int = (k equal 2\nthen 1\nelse 2\n) + 1\nif a {\nx = 1\n}\nelse {\nx = 2\n}\n}\n";
        let want = "function f(): void {\n    var t:string = k equal 2\n        then \"a\"\n        empty\n    var u:int = (k equal 2\n        then 1\n        else 2\n    ) + 1\n    if a {\n        x = 1\n    }\n    else {\n        x = 2\n    }\n}\n";
        assert_eq!(fix(src), want);
    }

    #[test]
    fn long_branches_are_split_into_the_indented_form() {
        let src = "function f(): void {\n    var f:F = k greater 2 then nameless(x:int): int { return x + k } else nameless(x:int): int { return x }\n    var p:int = (age greater 10 then (age greater 30 then aVeryLongValueNumberOne else 2) else 1) * 100\n    var s:string = ok then \"court\" else \"bref\"\n}\n";
        let want = "function f(): void {\n    var f:F = k greater 2\n        then nameless(x:int): int { return x + k }\n        else nameless(x:int): int { return x }\n    var p:int = (age greater 10\n        then (age greater 30\n            then aVeryLongValueNumberOne\n            else 2\n        )\n        else 1\n    ) * 100\n    var s:string = ok then \"court\" else \"bref\"\n}\n";
        assert_eq!(fix(src), want);
    }

    #[test]
    fn blank_lines_comments_and_continuations() {
        let src = "//note\n\n\n\n\nvar total:int = 1 +\n2\nvar s:string = \"http://x\"\n";
        assert_eq!(fix(src), "// note\n\n\nvar total:int = 1 +\n    2\nvar s:string = \"http://x\"\n");
    }
}
