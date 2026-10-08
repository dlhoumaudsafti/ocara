//! Lecture, lexing et parsing du fichier d'entrée.

use std::path::Path;

use crate::core::diagnostics::{span_file, Diagnostic};
use crate::parsing::ast::Program;
use crate::parsing::error::LexError;
use crate::parsing::{lexer::Lexer, parser::Parser, token};

pub fn parse_entry(input: &Path, dump: bool) -> Result<Program, Diagnostic> {
    let source = crate::core::source::read(input)
        .map_err(|e| Diagnostic::error(input, 0, 0, format!("cannot read '{}': {}", input.display(), e)))?;

    let tokens = Lexer::new(&source).tokenize().map_err(|e| {
        let (line, col) = match &e {
            LexError::UnexpectedChar(_, s)    => (s.line, s.col),
            LexError::UnterminatedString(s)   => (s.line, s.col),
            LexError::InvalidEscape(_, s)     => (s.line, s.col),
            LexError::IntegerOverflow(_, s)   => (s.line, s.col),
        };
        let msg = match &e {
            LexError::UnexpectedChar(ch, _)    => format!("unexpected character '{}'", ch),
            LexError::UnterminatedString(_)    => "unterminated string".into(),
            LexError::InvalidEscape(ch, _)     => format!("invalid escape sequence '\\{}'", ch),
            LexError::IntegerOverflow(raw, _)  => format!("integer too large: {}", raw),
        };
        Diagnostic::error(input, line, col, msg)
    })?;

    if dump {
        let non_eof: Vec<_> = tokens.iter()
            .filter(|t| t.kind != token::TokenKind::Eof)
            .collect();
        println!("=== TOKENS ({}) ===", non_eof.len());
        for tok in &non_eof { println!("{}", tok); }
        println!();
    }

    let program = Parser::new(tokens).parse_program()
        .map_err(|e| Diagnostic::error(&span_file(input, &e.span), e.span.line, e.span.col, e.message))?;

    if dump {
        println!("=== AST ===");
        println!("{:#?}", program);
        println!();
    }
    Ok(program)
}
