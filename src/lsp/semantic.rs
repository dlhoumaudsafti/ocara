//! Jetons sémantiques `ocaraWhen` (couleur dédiée des instructions bas
//! niveau du langage, associée par l'extension au scope TextMate de `when`) :
//! - `when` et ses opérateurs (`is`, `not`, `greater`, `smaller`, `equal`,
//!   `and`, `or`, `default`) sur la ligne d'une clause ;
//! - la déclaration `architecture` (`hexagonal`/`permissive`, propriétés
//!   `src`, `layers`, `shared` et noms de couche du bloc) ;
//! - le segment de couche des chemins `namespace`, `import` et `wiring`
//!   (`context.car.infra.db…`), selon l'architecture du projet (layers.rs).

use lsp_types::{SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend};

use super::layers::LayerSyntax;
use crate::parsing::lexer::Lexer;
use crate::parsing::token::{Token, TokenKind};

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend { token_types: vec![SemanticTokenType::new("ocaraWhen")], token_modifiers: Vec::new() }
}

const ARCH_KEYS: [&str; 6] = ["src", "layers", "shared", "infrastructure", "application", "domain"];

pub fn tokens(text: &str, layers: Option<&LayerSyntax>) -> SemanticTokens {
    let Ok(tokens) = Lexer::new(text).tokenize() else { return SemanticTokens::default() };
    let mut marked: Vec<&Token> = Vec::new();
    let mut when_line = 0;
    // Profondeur d'accolades dans le bloc `architecture` (0 = hors bloc).
    let mut arch_depth = 0usize;
    let mut arch_pending = false;
    for (i, tok) in tokens.iter().enumerate() {
        let next = tokens.get(i + 1).map(|t| &t.kind);
        let line = tok.span.line;
        match &tok.kind {
            TokenKind::When => { when_line = line; marked.push(tok); }
            TokenKind::Is | TokenKind::KwNot | TokenKind::KwGreater | TokenKind::KwSmaller | TokenKind::KwEqual
            | TokenKind::KwAnd | TokenKind::KwOr | TokenKind::Default if line == when_line => marked.push(tok),
            TokenKind::Ident(n) if n == "architecture" && arch_depth == 0
                && matches!(next, Some(TokenKind::Ident(m)) if m == "hexagonal" || m == "permissive") => {
                marked.push(tok);
                marked.push(&tokens[i + 1]);
                arch_pending = tokens.get(i + 2).is_some_and(|t| t.kind == TokenKind::LBrace);
            }
            TokenKind::LBrace if arch_pending || arch_depth > 0 => { arch_depth += 1; arch_pending = false; }
            TokenKind::RBrace if arch_depth > 0 => arch_depth -= 1,
            TokenKind::Ident(n) if arch_depth > 0 && ARCH_KEYS.contains(&n.as_str()) && next == Some(&TokenKind::Colon) => marked.push(tok),
            TokenKind::Namespace | TokenKind::Import | TokenKind::Wiring => {
                if let Some(seg) = layers.and_then(|l| l.layer_segment(&path_after(&tokens, i))) {
                    marked.push(seg);
                }
            }
            _ => {}
        }
    }
    marked.sort_by_key(|t| (t.span.line, t.span.col));
    marked.dedup_by_key(|t| (t.span.line, t.span.col));
    encode(text, &marked)
}

/// Segments `a.b.c` qui suivent le jeton `i` sur sa ligne.
fn path_after(tokens: &[Token], i: usize) -> Vec<&Token> {
    let line = tokens[i].span.line;
    let mut out = Vec::new();
    let mut j = i + 1;
    while let Some(t) = tokens.get(j).filter(|t| t.span.line == line) {
        if t.kind == TokenKind::Dot {
            j += 1;
            continue;
        }
        if matches!(t.kind, TokenKind::LitString(_) | TokenKind::As | TokenKind::From) || t.lexeme.is_empty() {
            break;
        }
        out.push(t);
        if tokens.get(j + 1).is_none_or(|n| n.kind != TokenKind::Dot) {
            break;
        }
        j += 1;
    }
    out
}

fn encode(text: &str, marked: &[&Token]) -> SemanticTokens {
    let lines: Vec<&str> = text.lines().collect();
    let (mut data, mut prev_line, mut prev_col) = (Vec::new(), 0u32, 0u32);
    for tok in marked {
        let Some(text_line) = lines.get(tok.span.line - 1) else { continue };
        let col = super::position::utf16_col(text_line, tok.span.col - 1) as u32;
        let line0 = (tok.span.line - 1) as u32;
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
    use crate::lsp::layers::LayerSyntax;

    fn spans(text: &str, layers: Option<&LayerSyntax>) -> Vec<(u32, u32, u32)> {
        tokens(text, layers).data.iter().map(|d| (d.delta_line, d.delta_start, d.length)).collect()
    }

    #[test]
    fn colors_when_and_its_operators_only() {
        let s = spans("when System::OS is not 'android' or System::BUILD is 'debug'\nfunction f(): bool { return true or false }\n", None);
        assert_eq!(s, vec![(0, 0, 4), (0, 16, 2), (0, 3, 3), (0, 14, 2), (0, 17, 2)]);
    }

    #[test]
    fn colors_architecture_block_and_layer_segments() {
        let text = "namespace context.car.infra.db\nimport context.car.domain.Car\narchitecture hexagonal {\n    src: context,\n    layers: { infrastructure: infra },\n}\n";
        let layers = LayerSyntax::new(vec!["context".into()], Vec::new(), ["infra", "app", "domain"].iter().map(|s| s.to_string()).collect());
        let s = spans(text, Some(&layers));
        // infra (l.1), domain (l.2), architecture + hexagonal, src, layers + infrastructure.
        assert_eq!(s, vec![(0, 22, 5), (1, 19, 6), (1, 0, 12), (0, 13, 9), (1, 4, 3), (1, 4, 6), (0, 10, 14)]);
    }
}
