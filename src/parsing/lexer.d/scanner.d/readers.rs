/// Lecteurs de tokens (string, template, number, ident)

use super::super::types::Lexer;
use crate::parsing::error::LexError;
use crate::parsing::token::{Span, Token, TokenKind};

/// Magnitude de `INT_MIN`, valable seulement derrière un `-`.
const INT_MIN_MAGNITUDE: &str = "9223372036854775808";

impl Lexer {
    /// Lit un littéral chaîne délimité par `"` ou `'`.
    /// Séquences d'échappement supportées : \n \t \r \" \' \\ \0
    pub(in crate::parsing::lexer) fn read_string(&mut self, start: Span, quote: char) -> Result<Token, LexError> {
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

    /// Chaîne template : `` `Bonjour ${name}, tu as ${age} ans !` ``
    /// Produit un token `LitTemplate(Vec<TemplatePart>)`.
    pub(in crate::parsing::lexer) fn read_template(&mut self, start: Span) -> Result<Token, LexError> {
        use crate::parsing::token::TemplatePart;
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
                    // Position réelle du premier caractère de l'expression —
                    // nécessaire pour recaler ses spans après le re-parsing
                    // isolé (voir ast.d::span_shift, parser.d::expressions).
                    let expr_start = self.span();
                    // Lire jusqu'au '}' en comptant les accolades imbriquées
                    let mut expr_src = String::new();
                    let mut depth: usize = 1;
                    loop {
                        match self.current() {
                            None => return Err(LexError::UnterminatedString(start)),
                            Some('{') => { depth += 1; expr_src.push('{'); self.advance(); }
                            // Chaîne de l'expression copiée telle quelle : ses
                            // accolades ne comptent pas (`${String::len("{x")}`
                            // donnait « unterminated string »).
                            Some(q @ ('"' | '\'')) => self.copy_quoted(q, &mut expr_src, &start)?,
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
                    parts.push(TemplatePart::ExprSrc(expr_src, expr_start));
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

    /// Lit un entier ou un flottant.
    ///
    /// Règle pour éviter la confusion avec `..` :
    /// le `.` n'est consommé comme séparateur décimal que si le caractère
    /// immédiatement suivant est un chiffre (ex. `1.5` ok, `0..5` → `0` `..` `5`).
    /// Copie dans `out` une chaîne délimitée par `quote` (guillemets compris,
    /// échappements conservés) d'une expression `${…}` de gabarit.
    fn copy_quoted(&mut self, quote: char, out: &mut String, start: &Span) -> Result<(), LexError> {
        out.push(quote);
        self.advance();
        loop {
            match self.current() {
                None => return Err(LexError::UnterminatedString(start.clone())),
                Some('\\') => {
                    out.push('\\');
                    self.advance();
                    if let Some(c) = self.current() {
                        out.push(c);
                        self.advance();
                    }
                }
                Some(c) => {
                    out.push(c);
                    self.advance();
                    if c == quote {
                        return Ok(());
                    }
                }
            }
        }
    }

    pub(in crate::parsing::lexer) fn read_number(&mut self, start: Span) -> Result<Token, LexError> {
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

        // `1.0e300` : notation à exposant non supportée (EBNF). Sans ce
        // contrôle, `e300` devenait un identifiant (« undefined symbol »).
        if matches!(self.current(), Some('e' | 'E'))
            && self.peek_next().is_some_and(|c| c.is_ascii_digit() || c == '+' || c == '-')
        {
            let mut raw = lexeme.clone();
            raw.push(self.current().unwrap_or('e'));
            while self.peek_next().is_some_and(|c| c.is_ascii_digit() || c == '+' || c == '-') {
                self.advance();
                raw.push(self.current().unwrap_or(' '));
            }
            return Err(LexError::ExponentNotation(raw, start));
        }

        let kind = if is_float {
            // Les flottants débordants deviennent ±inf, acceptable au niveau lexical.
            let v: f64 = lexeme.parse().unwrap_or(f64::INFINITY);
            TokenKind::LitFloat(v)
        } else {
            // `9223372036854775808` (2⁶³) : seulement la magnitude de
            // `-9223372036854775808` (INT_MIN), que le parseur exige précédée
            // d'un `-` (voir `Parser::int_literal`) ; aucun autre texte ne
            // donne `i64::MIN`.
            let v: i64 = match lexeme.parse() {
                Ok(v) => v,
                Err(_) if lexeme == INT_MIN_MAGNITUDE => i64::MIN,
                Err(_) => return Err(LexError::IntegerOverflow(lexeme.clone(), start.clone())),
            };
            TokenKind::LitInt(v)
        };

        Ok(Token::new(kind, lexeme, start))
    }

    /// Lit un identifiant ou un mot-clé.
    pub(in crate::parsing::lexer) fn read_ident_or_keyword(&mut self, start: Span) -> Token {
        let raw_start = self.pos;
        while self.current().map_or(false, |c| c.is_alphanumeric() || c == '_') {
            self.advance();
        }
        let lexeme: String = self.source[raw_start..self.pos].iter().collect();
        let kind = super::super::tokenizer::keyword_or_ident(&lexeme);
        Token::new(kind, lexeme, start)
    }
}
