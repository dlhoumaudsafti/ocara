// ─────────────────────────────────────────────────────────────────────────────
// `--fix` : mise en forme d'un fichier (R01, R02, R03, R04, R06, R10, R11).
// Le contenu des chaînes backtick multilignes n'est jamais modifié. R05
// (ligne trop longue) n'est pas corrigeable automatiquement.
// ─────────────────────────────────────────────────────────────────────────────

use crate::check::decl_assign_pos;
use crate::config::Config;
use crate::text::{comment_pos, indent_rule, leading_ws, line_states, scan_code, LineState};

pub fn fix_layout(content: &str, cfg: &Config) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let states = line_states(&lines);
    let mut out: Vec<String> = lines.iter().zip(&states)
        .map(|(line, state)| fix_line(line, *state, cfg))
        .collect();
    if cfg.indent {
        let as_str: Vec<&str> = out.iter().map(String::as_str).collect();
        let unit = indent_unit(&lines, &states, cfg);
        out = reindent(&as_str, &states, &unit);
    }
    if cfg.blank_lines_max > 0 {
        out = collapse_blank_lines(out, &states, cfg.blank_lines_max);
    }
    let mut result = out.join("\n");
    if !result.is_empty() && (content.ends_with('\n') || cfg.file_ends_newline) {
        result.push('\n');
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

/// Ligne prolongeant l'instruction précédente (`.chain()`, opérateur en
/// tête, ou ligne précédente terminée par un opérateur) : un niveau de plus.
fn is_continuation(trimmed: &str, prev_code: Option<&str>) -> bool {
    let starts = CONTINUATION_STARTS.iter().any(|p| trimmed.starts_with(p)) && !trimmed.starts_with("..");
    let ends = prev_code.is_some_and(|p| {
        CONTINUATION_ENDS.iter().any(|e| p.ends_with(e)) && !p.ends_with("++") && !p.ends_with("--") && !p.ends_with("==")
    });
    starts || ends
}

/// R01 : niveau = nombre de LIGNES portant une accolade/parenthèse/crochet
/// encore ouvert (plusieurs ouvrants sur une même ligne, comme
/// `route("/", "GET", nameless(...) {`, ne comptent qu'un niveau) ; les
/// fermants en tête de ligne sont retirés avant de mesurer.
fn reindent(lines: &[&str], states: &[LineState], unit: &str) -> Vec<String> {
    let mut stack: Vec<usize> = Vec::new();
    let mut in_bt = false;
    let mut prev_code: Option<String> = None;
    lines.iter().zip(states).enumerate().map(|(i, (line, state))| {
        let code_line = !state.starts_in_bt;
        let trimmed = line.trim_start();
        let lead_closers = if code_line {
            trimmed.chars().take_while(|c| matches!(c, '}' | ')' | ']')).count()
        } else { 0 };
        for _ in 0..lead_closers { stack.pop(); }
        let mut level = stack.windows(2).filter(|w| w[0] != w[1]).count() + usize::from(!stack.is_empty());
        let comment = scan_code(line, &mut in_bt, |pos, c| {
            if code_line && pos < leading_ws(line).len() + lead_closers { return; }
            match c {
                '{' | '(' | '[' => stack.push(i),
                '}' | ')' | ']' => { stack.pop(); }
                _ => {}
            }
        });
        if !code_line || trimmed.is_empty() { return line.to_string(); }
        if lead_closers == 0 && is_continuation(trimmed, prev_code.as_deref()) { level += 1; }
        let code = comment.map_or(*line, |c| &line[..c]).trim();
        if !code.is_empty() { prev_code = Some(code.to_string()); }
        format!("{}{}", unit.repeat(level), trimmed)
    }).collect()
}

/// R06 : au plus `max` lignes vides consécutives (hors backticks).
fn collapse_blank_lines(lines: Vec<String>, states: &[LineState], max: usize) -> Vec<String> {
    let mut blanks = 0usize;
    lines.into_iter().zip(states).filter_map(|(line, state)| {
        if line.trim().is_empty() && !state.inside_bt() {
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
    fn blank_lines_comments_and_continuations() {
        let src = "//note\n\n\n\n\nvar total:int = 1 +\n2\nvar s:string = \"http://x\"\n";
        assert_eq!(fix(src), "// note\n\n\nvar total:int = 1 +\n    2\nvar s:string = \"http://x\"\n");
    }
}
