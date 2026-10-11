/// Reprise sur erreur (mode tolérant, serveur de langage) : l'erreur est
/// notée et le parseur saute jusqu'au prochain point sûr — instruction
/// suivante (nouvelle ligne au même niveau d'accolades, ou fin du bloc),
/// membre suivant, déclaration suivante. Hors mode tolérant, l'erreur est
/// renvoyée telle quelle (le CLI s'arrête à la première).

use crate::parsing::ast::Program;
use crate::parsing::token::TokenKind;
use super::types::{ParseError, ParseResult, Parser};

impl Parser {
    /// Programme partiel et toutes les erreurs de syntaxe.
    pub fn parse_program_recovering(&mut self) -> (Program, Vec<ParseError>) {
        self.recover = true;
        let program = match self.parse_program() {
            Ok(program) => program,
            Err(e) => {
                self.errors.push(e);
                Program::new()
            }
        };
        (program, std::mem::take(&mut self.errors))
    }

    /// Instruction en erreur commencée à la position `start`.
    pub(super) fn recover_stmt(&mut self, err: ParseError, start: usize) -> ParseResult<()> {
        self.note(err, start)?;
        let line = self.tokens[start].span.line;
        self.skip_until(|p, depth| depth == 0 && (p.check_exact(&TokenKind::RBrace) || p.current().span.line > line));
        Ok(())
    }

    /// Membre de classe, de generic ou de module en erreur.
    pub(super) fn recover_member(&mut self, err: ParseError, start: usize) -> ParseResult<()> {
        self.note(err, start)?;
        self.skip_until(|p, depth| depth == 0 && (p.check_exact(&TokenKind::RBrace) || p.at_member_start()));
        Ok(())
    }

    /// Déclaration de premier niveau en erreur.
    pub(super) fn recover_top(&mut self, err: ParseError, start: usize) -> ParseResult<()> {
        self.note(err, start)?;
        self.skip_until(|p, depth| depth == 0 && p.at_top_level_start());
        Ok(())
    }

    /// Note l'erreur et garantit d'avancer d'au moins un jeton ; refusé hors
    /// mode tolérant et en fin de fichier.
    fn note(&mut self, err: ParseError, start: usize) -> ParseResult<()> {
        if !self.recover || self.check_exact(&TokenKind::Eof) {
            return Err(err);
        }
        self.errors.push(err);
        if self.pos == start {
            self.advance();
        }
        Ok(())
    }

    /// Avance jusqu'à `stop` (évalué avec la profondeur d'accolades,
    /// parenthèses et crochets ouverts depuis le point de départ).
    fn skip_until(&mut self, stop: impl Fn(&Parser, i32) -> bool) {
        let mut depth: i32 = 0;
        while !self.check_exact(&TokenKind::Eof) && !stop(self, depth) {
            match self.peek_kind() {
                TokenKind::LBrace | TokenKind::LParen | TokenKind::LBracket => depth += 1,
                TokenKind::RParen | TokenKind::RBracket => depth = (depth - 1).max(0),
                TokenKind::RBrace if depth == 0 => return,
                TokenKind::RBrace => depth -= 1,
                _ => {}
            }
            self.advance();
        }
    }

    fn at_member_start(&self) -> bool {
        matches!(self.peek_kind(),
            TokenKind::Public | TokenKind::Private | TokenKind::Protected | TokenKind::Static
            | TokenKind::Method | TokenKind::Property | TokenKind::Init | TokenKind::Const | TokenKind::Async)
    }

    fn at_top_level_start(&self) -> bool {
        matches!(self.peek_kind(),
            TokenKind::Import | TokenKind::Runtime | TokenKind::Init | TokenKind::Main | TokenKind::Error
            | TokenKind::Success | TokenKind::Exit | TokenKind::Const | TokenKind::Module | TokenKind::Enum
            | TokenKind::Class | TokenKind::Struct | TokenKind::Generic | TokenKind::Interface
            | TokenKind::Function | TokenKind::Async)
    }
}
