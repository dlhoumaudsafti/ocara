//! Jetons sémantiques : `when` et ses opérateurs (`is`, `not`, `greater`,
//! `smaller`, `equal`, `and`, `or`, `default`) sur la ligne d'une clause
//! reçoivent le type `ocaraWhen`, que l'extension associe au scope TextMate
//! de la clause (même couleur dédiée).

use lsp_types::{SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend};

use crate::parsing::lexer::Lexer;
use crate::parsing::token::TokenKind;

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend { token_types: vec![SemanticTokenType::new("ocaraWhen")], token_modifiers: Vec::new() }
}

pub fn tokens(text: &str) -> SemanticTokens {
    let Ok(tokens) = Lexer::new(text).tokenize() else { return SemanticTokens::default() };
    let lines: Vec<&str> = text.lines().collect();
    let mut data = Vec::new();
    let (mut when_line, mut prev_line, mut prev_col) = (0, 0, 0);
    for tok in &tokens {
        let line = tok.span.line;
        let is_when = tok.kind == TokenKind::When;
        if is_when {
            when_line = line;
        }
        let operator = matches!(tok.kind,
            TokenKind::Is | TokenKind::KwNot | TokenKind::KwGreater | TokenKind::KwSmaller | TokenKind::KwEqual
            | TokenKind::KwAnd | TokenKind::KwOr | TokenKind::Default);
        if !is_when && !(operator && line == when_line) {
            continue;
        }
        let Some(text_line) = lines.get(line - 1) else { continue };
        let col = super::position::utf16_col(text_line, tok.span.col - 1) as u32;
        let line0 = (line - 1) as u32;
        let delta_line = line0 - prev_line;
        data.push(SemanticToken {
            delta_line,
            delta_start: if delta_line == 0 { col - prev_col } else { col },
            length: tok.lexeme.chars().map(char::len_utf16).sum::<usize>() as u32,
            token_type: 0,
            token_modifiers_bitset: 0,
        });
        (prev_line, prev_col) = (line0, col);
    }
    SemanticTokens { result_id: None, data }
}

#[cfg(test)]
mod tests {
    use super::tokens;

    #[test]
    fn colors_when_and_its_operators_only() {
        let t = tokens("when System::OS is not 'android' or System::BUILD is 'debug'\nfunction f(): bool { return true or false }\n");
        let starts: Vec<(u32, u32, u32)> = t.data.iter().map(|d| (d.delta_line, d.delta_start, d.length)).collect();
        assert_eq!(starts, vec![(0, 0, 4), (0, 16, 2), (0, 3, 3), (0, 14, 2), (0, 17, 2)]);
    }
}
