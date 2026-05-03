/// Gestion des espaces et commentaires

use super::super::types::Lexer;

impl Lexer {
    /// Saute les espaces, tabulations, sauts de ligne et commentaires.
    pub(in crate::parsing::lexer) fn skip_whitespace_and_comments(&mut self) {
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
}
