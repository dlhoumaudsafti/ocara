use crate::error::LexError;
use crate::token::{Span, Token, TokenKind};

// ─────────────────────────────────────────────────────────────────────────────
// Lexer
// ─────────────────────────────────────────────────────────────────────────────

pub struct Lexer {
    source: Vec<char>,
    pos:    usize,
    line:   usize,
    col:    usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            pos:    0,
            line:   1,
            col:    1,
        }
    }

    // ── Primitives ───────────────────────────────────────────────────────────

    /// Caractère courant (sans avancer).
    fn current(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    /// Caractère suivant (look-ahead 1, sans avancer).
    fn peek_next(&self) -> Option<char> {
        self.source.get(self.pos + 1).copied()
    }

    /// Avance d'un caractère et met à jour ligne/colonne.
    fn advance(&mut self) -> Option<char> {
        let ch = self.source.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    /// Retourne le `Span` du prochain caractère à lire.
    fn span(&self) -> Span {
        Span::new(self.line, self.col)
    }

    // ── Saut de blancs & commentaires ────────────────────────────────────────

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.current() {
                // Espaces/tabulations/sauts de ligne
                Some(c) if c.is_whitespace() => { self.advance(); }

                // Commentaire ligne  //
                Some('/') if self.peek_next() == Some('/') => {
                    self.advance(); // '/'
                    self.advance(); // '/'
                    while !matches!(self.current(), None | Some('\n')) {
                        self.advance();
                    }
                }

                _ => break,
            }
        }
    }

    // ── Lecteurs spécialisés ─────────────────────────────────────────────────

    /// Lit un littéral chaîne délimité par `"`.
    /// Séquences d'échappement supportées : \n \t \r \" \' \\ \0
    fn read_string(&mut self, start: Span, quote: char) -> Result<Token, LexError> {
        let raw_start = self.pos;
        self.advance(); // guillemet ouvrant

        let mut value = String::new();

        loop {
            match self.current() {
                None | Some('\n') => {
                    return Err(LexError::UnterminatedString(start));
                }
                Some(c) if c == quote => {
                    self.advance(); // guillemet fermant
                    break;
                }
                Some('\\') => {
                    self.advance(); // '\'
                    let esc_span = self.span();
                    let escaped = match self.advance() {
                        Some('n')  => '\n',
                        Some('t')  => '\t',
                        Some('r')  => '\r',
                        Some('"')  => '"',
                        Some('\'') => '\'',
                        Some('\\') => '\\',
                        Some('0')  => '\0',
                        Some(c)    => return Err(LexError::InvalidEscape(c, esc_span)),
                        None       => return Err(LexError::UnterminatedString(start)),
                    };
                    value.push(escaped);
                }
                Some(c) => {
                    value.push(c);
                    self.advance();
                }
            }
        }

        let lexeme: String = self.source[raw_start..self.pos].iter().collect();
        Ok(Token::new(TokenKind::LitString(value), lexeme, start))
    }

    /// Lit un entier ou un flottant.
    ///
    /// Chaîne template : `` `Bonjour ${name}, tu as ${age} ans !` ``
    /// Produit un token `LitTemplate(Vec<TemplatePart>)`.
    fn read_template(&mut self, start: Span) -> Result<Token, LexError> {
        use crate::token::TemplatePart;
        self.advance(); // backtick ouvrant

        let mut parts: Vec<TemplatePart> = Vec::new();
        let mut literal = String::new();

        loop {
            match self.current() {
                None => return Err(LexError::UnterminatedString(start)),
                Some('`') => {
                    self.advance();
                    break;
                }
                Some('$') if self.peek_next() == Some('{') => {
                    // Flush le texte accumulé
                    if !literal.is_empty() {
                        parts.push(TemplatePart::Literal(std::mem::take(&mut literal)));
                    }
                    self.advance(); // '$'
                    self.advance(); // '{'
                    // Lire jusqu'au '}' en comptant les accolades imbriquées
                    let mut expr_src = String::new();
                    let mut depth: usize = 1;
                    loop {
                        match self.current() {
                            None => return Err(LexError::UnterminatedString(start)),
                            Some('{') => { depth += 1; expr_src.push('{'); self.advance(); }
                            Some('}') => {
                                depth -= 1;
                                if depth == 0 {
                                    self.advance(); // '}'
                                    break;
                                }
                                expr_src.push('}');
                                self.advance();
                            }
                            Some(c) => { expr_src.push(c); self.advance(); }
                        }
                    }
                    parts.push(TemplatePart::ExprSrc(expr_src));
                }
                Some('\\') => {
                    self.advance();
                    let esc_span = self.span();
                    let c = match self.advance() {
                        Some('n')  => '\n',
                        Some('t')  => '\t',
                        Some('r')  => '\r',
                        Some('`')  => '`',
                        Some('\\') => '\\',
                        Some('$')  => '$',
                        Some(c)    => return Err(LexError::InvalidEscape(c, esc_span)),
                        None       => return Err(LexError::UnterminatedString(start)),
                    };
                    literal.push(c);
                }
                Some(c) => { literal.push(c); self.advance(); }
            }
        }

        if !literal.is_empty() {
            parts.push(TemplatePart::Literal(literal));
        }

        Ok(Token::new(TokenKind::LitTemplate(parts), "`...`", start))
    }

    /// Règle pour éviter la confusion avec `..` :
    /// le `.` n'est consommé comme séparateur décimal que si le caractère
    /// immédiatement suivant est un chiffre (ex. `1.5` ok, `0..5` → `0` `..` `5`).
    fn read_number(&mut self, start: Span) -> Result<Token, LexError> {
        let raw_start = self.pos;
        let mut is_float = false;

        // Partie entière
        while self.current().map_or(false, |c| c.is_ascii_digit()) {
            self.advance();
        }

        // Partie fractionnaire (`.` suivi d'un chiffre)
        if self.current() == Some('.')
            && self.peek_next().map_or(false, |c| c.is_ascii_digit())
        {
            is_float = true;
            self.advance(); // '.'
            while self.current().map_or(false, |c| c.is_ascii_digit()) {
                self.advance();
            }
        }

        let lexeme: String = self.source[raw_start..self.pos].iter().collect();

        let kind = if is_float {
            // Les flottants débordants deviennent ±inf, acceptable au niveau lexical.
            let v: f64 = lexeme.parse().unwrap_or(f64::INFINITY);
            TokenKind::LitFloat(v)
        } else {
            let v: i64 = lexeme.parse().map_err(|_| {
                LexError::IntegerOverflow(lexeme.clone(), start.clone())
            })?;
            TokenKind::LitInt(v)
        };

        Ok(Token::new(kind, lexeme, start))
    }

    /// Lit un identifiant ou un mot-clé.
    fn read_ident_or_keyword(&mut self, start: Span) -> Token {
        let raw_start = self.pos;
        while self.current().map_or(false, |c| c.is_alphanumeric() || c == '_') {
            self.advance();
        }
        let lexeme: String = self.source[raw_start..self.pos].iter().collect();
        let kind = Self::keyword_or_ident(&lexeme);
        Token::new(kind, lexeme, start)
    }

    // ── Mapping mot-clé ──────────────────────────────────────────────────────

    fn keyword_or_ident(s: &str) -> TokenKind {
        match s {
            "import"     => TokenKind::Import,
            "as"         => TokenKind::As,
            "var"        => TokenKind::Var,
            "scoped"     => TokenKind::Scoped,
            "property"   => TokenKind::Property,
            "const"      => TokenKind::Const,
            "function"   => TokenKind::Function,
            "method"     => TokenKind::Method,
            "class"      => TokenKind::Class,
            "interface"  => TokenKind::Interface,
            "extends"    => TokenKind::Extends,
            "implements" => TokenKind::Implements,
            "init"       => TokenKind::Init,
            "public"     => TokenKind::Public,
            "private"    => TokenKind::Private,
            "protected"  => TokenKind::Protected,
            "static"     => TokenKind::Static,
            "if"         => TokenKind::If,
            "elseif"     => TokenKind::Elseif,
            "else"       => TokenKind::Else,
            "switch"     => TokenKind::Switch,
            "default"    => TokenKind::Default,
            "match"      => TokenKind::Match,
            "while"      => TokenKind::While,
            "for"        => TokenKind::For,
            "in"         => TokenKind::In,
            "return"     => TokenKind::Return,
            "use"        => TokenKind::Use,
            "break"      => TokenKind::Break,
            "continue"   => TokenKind::Continue,
            "try"        => TokenKind::Try,
            "on"         => TokenKind::On,
            "is"         => TokenKind::Is,
            "raise"      => TokenKind::Raise,
            "self"       => TokenKind::SelfKw,
            "int"        => TokenKind::TInt,
            "float"      => TokenKind::TFloat,
            "string"     => TokenKind::TString,
            "bool"       => TokenKind::TBool,
            "mixed"      => TokenKind::TMixed,
            "map"        => TokenKind::TMap,
            "void"       => TokenKind::TVoid,
            "true"       => TokenKind::LitTrue,
            "false"      => TokenKind::LitFalse,
            "null"       => TokenKind::LitNull,
            _            => TokenKind::Ident(s.to_string()),
        }
    }

    // ── Tokeniseur principal ─────────────────────────────────────────────────

    /// Produit le prochain `Token`.
    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace_and_comments();

        let span = self.span();

        let ch = match self.current() {
            None    => return Ok(Token::new(TokenKind::Eof, "", span)),
            Some(c) => c,
        };

        // Littéral chaîne (double ou simple guillemets)
        if ch == '"' || ch == '\'' {
            return self.read_string(span, ch);
        }

        // Chaîne template : `...${expr}...`
        if ch == '`' {
            return self.read_template(span);
        }

        // Littéral numérique
        if ch.is_ascii_digit() {
            return self.read_number(span);
        }

        // Identifiant / mot-clé
        if ch.is_alphabetic() || ch == '_' {
            return Ok(self.read_ident_or_keyword(span));
        }

        // Opérateurs & ponctuation
        // On consomme le premier caractère ; les opérateurs bi-caractères
        // vérifient self.current() (qui est désormais le 2e caractère).
        self.advance();

        let kind = match ch {
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,

            // !  ou  !=
            '!' => match self.current() {
                Some('=') => { self.advance(); TokenKind::BangEq }
                _         => TokenKind::Bang,
            },

            // =  ou  ==  ou  =>
            '=' => match self.current() {
                Some('=') => { self.advance(); TokenKind::EqEq   }
                Some('>') => { self.advance(); TokenKind::Arrow   }
                _         => TokenKind::Eq,
            },

            // <  ou  <=
            '<' => match self.current() {
                Some('=') => { self.advance(); TokenKind::LtEq }
                _         => TokenKind::Lt,
            },

            // >  ou  >=
            '>' => match self.current() {
                Some('=') => { self.advance(); TokenKind::GtEq }
                _         => TokenKind::Gt,
            },

            // &&
            '&' => match self.current() {
                Some('&') => { self.advance(); TokenKind::And }
                _         => return Err(LexError::UnexpectedChar('&', span)),
            },

            // || ou |
            '|' => match self.current() {
                Some('|') => { self.advance(); TokenKind::Or }
                _         => TokenKind::Pipe,
            },

            // :  ou  ::
            ':' => match self.current() {
                Some(':') => { self.advance(); TokenKind::ColonColon }
                _         => TokenKind::Colon,
            },

            // .  ou  ..
            '.' => match self.current() {
                Some('.') => { self.advance(); TokenKind::DotDot }
                _         => TokenKind::Dot,
            },

            c => return Err(LexError::UnexpectedChar(c, span)),
        };

        // Reconstruction du lexème pour les opérateurs/ponctuation
        let lexeme = lexeme_str(&kind, ch);
        Ok(Token::new(kind, lexeme, span))
    }

    /// Tokenise tout le source et retourne la liste complète.
    /// Le dernier élément est toujours `TokenKind::Eof`.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let done = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if done { break; }
        }
        Ok(tokens)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Retourne le lexème textuel d'un token opérateur/ponctuation.
fn lexeme_str(kind: &TokenKind, first: char) -> String {
    match kind {
        TokenKind::BangEq     => "!=".into(),
        TokenKind::EqEq       => "==".into(),
        TokenKind::Arrow      => "=>".into(),
        TokenKind::LtEq       => "<=".into(),
        TokenKind::GtEq       => ">=".into(),
        TokenKind::And        => "&&".into(),
        TokenKind::Or         => "||".into(),
        TokenKind::ColonColon => "::".into(),
        TokenKind::DotDot     => "..".into(),
        _                     => first.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
        let tks = kinds("== != <= >= && || => ::");
        assert_eq!(tks[0], TokenKind::EqEq);
        assert_eq!(tks[1], TokenKind::BangEq);
        assert_eq!(tks[2], TokenKind::LtEq);
        assert_eq!(tks[3], TokenKind::GtEq);
        assert_eq!(tks[4], TokenKind::And);
        assert_eq!(tks[5], TokenKind::Or);
        assert_eq!(tks[6], TokenKind::Arrow);
        assert_eq!(tks[7], TokenKind::ColonColon);
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
