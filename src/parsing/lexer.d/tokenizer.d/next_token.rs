/// Tokenization principale

use crate::parsing::error::LexError;
use crate::parsing::lexer::Lexer;
use crate::parsing::token::{Token, TokenKind};

impl Lexer {
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

            // ! ou != ou !==
            '!' => match self.current() {
                Some('=') => {
                    self.advance();
                    match self.current() {
                        Some('=') => { self.advance(); TokenKind::BangEqEq }
                        _         => TokenKind::BangEq
                    }
                }
                _ => return Err(LexError::UnexpectedChar('!', span)),
            },

            // =  ou  ==  ou === ou  =>
            '=' => match self.current() {
                Some('=') => {
                    self.advance();
                    match self.current() {
                        Some('=') => { self.advance(); TokenKind::EqEqEq }
                        _         => TokenKind::EqEq
                    }
                }
                Some('>') => { self.advance(); TokenKind::Arrow   }
                _         => TokenKind::Eq,
            },

            // <  ou  <= ou <==
            '<' => match self.current() {
                Some('=') => {
                    self.advance();
                    match self.current() {
                        Some('=') => { self.advance(); TokenKind::LtEqEq }
                        _         => TokenKind::LtEq
                    }
                }
                _ => TokenKind::Lt,
            },

            // >  ou  >= ou >==
            '>' => match self.current() {
                Some('=') => {
                    self.advance();
                    match self.current() {
                        Some('=') => { self.advance(); TokenKind::GtEqEq }
                        _         => TokenKind::GtEq
                    }
                }
                _ => TokenKind::Gt,
            },

            // & interdit (utiliser 'and')
            '&' => return Err(LexError::UnexpectedChar('&', span)),

            // | ou || interdit (| = union de type, || = utiliser 'or')
            '|' => match self.current() {
                Some('|') => return Err(LexError::UnexpectedChar('|', span)),
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
        let lexeme = super::super::helpers::lexeme_str(&kind, ch);
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
