/// Tests unitaires du lexer

#[cfg(test)]
mod tests {
    use crate::parsing::lexer::Lexer;
    use crate::parsing::token::{Span, TokenKind};
    use crate::parsing::error::LexError;

    fn kinds(src: &str) -> Vec<TokenKind> {
        Lexer::new(src)
            .tokenize()
            .expect("lexer error")
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    // ── Mots-clés ────────────────────────────────────────────────────────────

    #[test]
    fn test_keywords() {
        let tks = kinds("import as var scoped const function method class interface");
        assert_eq!(
            tks,
            vec![
                TokenKind::Import, TokenKind::As, TokenKind::Var, TokenKind::Scoped,
                TokenKind::Const, TokenKind::Function, TokenKind::Method, TokenKind::Class, TokenKind::Interface,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_visibility_and_control() {
        let tks = kinds("public private protected if elseif else while for in return");
        assert_eq!(tks[0], TokenKind::Public);
        assert_eq!(tks[1], TokenKind::Private);
        assert_eq!(tks[2], TokenKind::Protected);
        assert_eq!(tks[3], TokenKind::If);
        assert_eq!(tks[4], TokenKind::Elseif);
        assert_eq!(tks[5], TokenKind::Else);
        assert_eq!(tks[6], TokenKind::While);
        assert_eq!(tks[7], TokenKind::For);
        assert_eq!(tks[8], TokenKind::In);
        assert_eq!(tks[9], TokenKind::Return);
    }

    // ── Types primitifs ───────────────────────────────────────────────────────

    #[test]
    fn test_type_keywords() {
        let tks = kinds("int float string bool mixed map void");
        assert_eq!(
            tks,
            vec![
                TokenKind::TInt, TokenKind::TFloat, TokenKind::TString,
                TokenKind::TBool, TokenKind::TMixed, TokenKind::TMap, TokenKind::TVoid,
                TokenKind::Eof,
            ]
        );
    }

    // ── Littéraux ─────────────────────────────────────────────────────────────

    #[test]
    fn test_integer() {
        let tks = kinds("42");
        assert_eq!(tks[0], TokenKind::LitInt(42));
    }

    #[test]
    fn test_float() {
        let tks = kinds("3.14");
        assert_eq!(tks[0], TokenKind::LitFloat(3.14));
    }

    #[test]
    fn test_string_simple() {
        let tks = kinds(r#""hello world""#);
        assert_eq!(tks[0], TokenKind::LitString("hello world".into()));
    }

    #[test]
    fn test_string_escapes() {
        let tks = kinds(r#""line\nnext\ttab""#);
        assert_eq!(tks[0], TokenKind::LitString("line\nnext\ttab".into()));
    }

    #[test]
    fn test_booleans() {
        let tks = kinds("true false");
        assert_eq!(tks[0], TokenKind::LitTrue);
        assert_eq!(tks[1], TokenKind::LitFalse);
    }

    // ── Opérateurs mono et bi-caractères ─────────────────────────────────────

    #[test]
    fn test_single_char_ops() {
        let tks = kinds("+ - * / %");
        assert_eq!(tks[0], TokenKind::Plus);
        assert_eq!(tks[1], TokenKind::Minus);
        assert_eq!(tks[2], TokenKind::Star);
        assert_eq!(tks[3], TokenKind::Slash);
        assert_eq!(tks[4], TokenKind::Percent);
    }

    #[test]
    fn test_multi_char_ops() {
        let tks = kinds("== != <= >= => ::");
        assert_eq!(tks[0], TokenKind::EqEq);
        assert_eq!(tks[1], TokenKind::BangEq);
        assert_eq!(tks[2], TokenKind::LtEq);
        assert_eq!(tks[3], TokenKind::GtEq);
        assert_eq!(tks[4], TokenKind::Arrow);
        assert_eq!(tks[5], TokenKind::ColonColon);
    }

    #[test]
    fn test_eq_vs_eqeq() {
        let tks = kinds("= ==");
        assert_eq!(tks[0], TokenKind::Eq);
        assert_eq!(tks[1], TokenKind::EqEq);
    }

    #[test]
    fn test_colon_vs_coloncolon() {
        let tks = kinds(": ::");
        assert_eq!(tks[0], TokenKind::Colon);
        assert_eq!(tks[1], TokenKind::ColonColon);
    }

    // ── Opérateur range `..`  vs  point `.`  vs  flottant ────────────────────

    #[test]
    fn test_dotdot_range() {
        // 0..5 → LitInt(0)  DotDot  LitInt(5)
        let tks = kinds("0..5");
        assert_eq!(tks[0], TokenKind::LitInt(0));
        assert_eq!(tks[1], TokenKind::DotDot);
        assert_eq!(tks[2], TokenKind::LitInt(5));
    }

    #[test]
    fn test_float_not_range() {
        // 1.5 → LitFloat(1.5)
        let tks = kinds("1.5");
        assert_eq!(tks[0], TokenKind::LitFloat(1.5));
    }

    #[test]
    fn test_member_access() {
        // user.age → Ident  Dot  Ident
        let tks = kinds("user.age");
        assert_eq!(tks[0], TokenKind::Ident("user".into()));
        assert_eq!(tks[1], TokenKind::Dot);
        assert_eq!(tks[2], TokenKind::Ident("age".into()));
    }

    // ── Commentaires ─────────────────────────────────────────────────────────

    #[test]
    fn test_line_comment_skipped() {
        let tks = kinds("42 // ceci est ignoré\n99");
        assert_eq!(tks[0], TokenKind::LitInt(42));
        assert_eq!(tks[1], TokenKind::LitInt(99));
        assert_eq!(tks[2], TokenKind::Eof);
    }

    // ── Span / positions ─────────────────────────────────────────────────────

    #[test]
    fn test_span_tracking() {
        let mut lexer = Lexer::new("import\nfunction");
        let t1 = lexer.next_token().unwrap();
        let t2 = lexer.next_token().unwrap();
        assert_eq!(t1.span, Span::new(1, 1));
        assert_eq!(t2.span, Span::new(2, 1));
    }

    // ── Erreurs ───────────────────────────────────────────────────────────────

    #[test]
    fn test_unterminated_string() {
        let err = Lexer::new("\"hello").tokenize().unwrap_err();
        assert!(matches!(err, LexError::UnterminatedString(_)));
    }

    #[test]
    fn test_invalid_escape() {
        let err = Lexer::new(r#""\q""#).tokenize().unwrap_err();
        assert!(matches!(err, LexError::InvalidEscape('q', _)));
    }

    #[test]
    fn test_unexpected_char() {
        let err = Lexer::new("@").tokenize().unwrap_err();
        assert!(matches!(err, LexError::UnexpectedChar('@', _)));
    }
}
