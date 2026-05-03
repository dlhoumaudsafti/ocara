/// Fonctions utilitaires pour le lexer

use crate::parsing::token::TokenKind;

/// Retourne le lexème textuel d'un token opérateur/ponctuation.
pub(super) fn lexeme_str(kind: &TokenKind, first: char) -> String {
    match kind {
        TokenKind::BangEq     => "!=".into(),
        TokenKind::BangEqEq   => "!==".into(),
        TokenKind::EqEq       => "==".into(),
        TokenKind::EqEqEq     => "===".into(),
        TokenKind::Lt         => "<".into(),
        TokenKind::LtEq       => "<=".into(),
        TokenKind::LtEqEq     => "<==".into(),
        TokenKind::Gt         => ">".into(),
        TokenKind::GtEq       => ">=".into(),
        TokenKind::GtEqEq     => ">==".into(),
        TokenKind::Arrow      => "=>".into(),
        TokenKind::ColonColon => "::".into(),
        TokenKind::DotDot     => "..".into(),
        _                     => first.to_string(),
    }
}
