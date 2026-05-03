/// Parsing des statements

use crate::parsing::ast::*;
use crate::parsing::token::TokenKind;
use super::types::{Parser, ParseError, ParseResult};

impl Parser {
    pub(super) fn parse_block(&mut self) -> ParseResult<Block> {
        let span = self.span();
        self.eat(&TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            stmts.push(self.parse_stmt()?);
        }
        self.eat(&TokenKind::RBrace)?;
        Ok(Block { stmts, span })
    }

    pub(super) fn parse_stmt(&mut self) -> ParseResult<Stmt> {
        match self.peek_kind().clone() {
            TokenKind::Var | TokenKind::Scoped => self.parse_var_decl(),
            TokenKind::Const               => self.parse_const_stmt(),
            TokenKind::If                  => self.parse_if(),
            TokenKind::Switch              => self.parse_switch(),
            TokenKind::While               => self.parse_while(),
            TokenKind::For                 => self.parse_for(),
            TokenKind::Return              => self.parse_return(),
            TokenKind::Break               => {
                let span = self.span();
                self.advance();
                Ok(Stmt::Break { span })
            }
            TokenKind::Continue            => {
                let span = self.span();
                self.advance();
                Ok(Stmt::Continue { span })
            }
            TokenKind::Try                 => self.parse_try(),
            TokenKind::Raise               => self.parse_raise(),
            _ => {
                let expr = self.parse_expr()?;
                // Affectation : `target = value`
                if self.check_exact(&TokenKind::Eq) {
                    let span = self.span();
                    self.advance();
                    let value = self.parse_expr()?;
                    return Ok(Stmt::Assign { target: expr, value, span });
                }
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_var_decl(&mut self) -> ParseResult<Stmt> {
        let span = self.span();
        let mutable = match self.peek_kind() {
            TokenKind::Var    => { self.advance(); true  }
            _                 => { self.advance(); true  } // Scoped : mutable, scope = durée de vie du bloc
        };
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.eat(&TokenKind::Eq)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Var { name, ty, value, mutable, span })
    }

    fn parse_const_stmt(&mut self) -> ParseResult<Stmt> {
        let span = self.span();
        self.eat(&TokenKind::Const)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.eat(&TokenKind::Eq)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Const { name, ty, value, span })
    }

    fn parse_if(&mut self) -> ParseResult<Stmt> {
        let span = self.span();
        self.eat(&TokenKind::If)?;
        let condition = self.parse_expr()?;
        let then_block = self.parse_block()?;

        let mut elseif = Vec::new();
        while self.check_exact(&TokenKind::Elseif) {
            self.advance();
            let cond = self.parse_expr()?;
            let blk = self.parse_block()?;
            elseif.push((cond, blk));
        }

        let else_block = if self.check_exact(&TokenKind::Else) {
            self.advance();
            Some(self.parse_block()?)
        } else {
            None
        };

        Ok(Stmt::If { condition, then_block, elseif, else_block, span })
    }

    fn parse_switch(&mut self) -> ParseResult<Stmt> {
        let span = self.span();
        self.eat(&TokenKind::Switch)?;
        let subject = self.parse_expr()?;
        self.eat(&TokenKind::LBrace)?;

        let mut cases = Vec::new();
        let mut default = None;

        while !self.check_exact(&TokenKind::RBrace) {
            let case_span = self.span();
            if self.check_exact(&TokenKind::Default) {
                self.advance();
                default = Some(self.parse_block()?);
            } else {
                let pattern = self.parse_literal()?;
                let body = self.parse_block()?;
                cases.push(SwitchCase { pattern, body, span: case_span });
            }
        }

        self.eat(&TokenKind::RBrace)?;
        Ok(Stmt::Switch { subject, cases, default, span })
    }

    fn parse_while(&mut self) -> ParseResult<Stmt> {
        let span = self.span();
        self.eat(&TokenKind::While)?;
        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::While { condition, body, span })
    }

    fn parse_for(&mut self) -> ParseResult<Stmt> {
        let span = self.span();
        self.eat(&TokenKind::For)?;

        let first = self.eat_ident()?.0;

        // `for k => v in expr`
        if self.check_exact(&TokenKind::Arrow) {
            self.advance();
            let val = self.eat_ident()?.0;
            self.eat(&TokenKind::In)?;
            let iter = self.parse_expr()?;
            let body = self.parse_block()?;
            return Ok(Stmt::ForMap { key: first, value: val, iter, body, span });
        }

        // `for i in expr`
        self.eat(&TokenKind::In)?;
        let iter = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::ForIn { var: first, iter, body, span })
    }

    fn parse_return(&mut self) -> ParseResult<Stmt> {
        let span = self.span();
        self.eat(&TokenKind::Return)?;

        // `return` sans valeur si on tombe sur `}`
        let value = if self.check_exact(&TokenKind::RBrace) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        Ok(Stmt::Return { value, span })
    }

    fn parse_try(&mut self) -> ParseResult<Stmt> {
        let span = self.span();
        self.eat(&TokenKind::Try)?;
        let body = self.parse_block()?;

        // Au moins une clause `on` obligatoire
        let mut handlers = Vec::new();
        while self.check_exact(&TokenKind::On) {
            let h_span = self.span();
            self.advance(); // consomme `on`

            // binding: identifiant de la variable d'erreur
            let (binding, _) = self.eat_ident()?;

            // filtre optionnel : `is ClassName`
            let class_filter = if self.check_exact(&TokenKind::Is) {
                self.advance(); // consomme `is`
                let (class_name, _) = self.eat_ident()?;
                Some(class_name)
            } else {
                None
            };

            let handler_body = self.parse_block()?;
            handlers.push(OnClause { binding, class_filter, body: handler_body, span: h_span });
        }

        if handlers.is_empty() {
            return Err(ParseError {
                message: "try sans clause `on`".into(),
                span,
            });
        }

        Ok(Stmt::Try { body, handlers, span })
    }

    fn parse_raise(&mut self) -> ParseResult<Stmt> {
        let span = self.span();
        self.eat(&TokenKind::Raise)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Raise { value, span })
    }

    // ── Littéral isolé (pour patterns) ───────────────────────────────────────

    pub(super) fn parse_literal(&mut self) -> ParseResult<Literal> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::LitInt(n)    => { self.advance(); Ok(Literal::Int(n)) }
            TokenKind::LitFloat(f)  => { self.advance(); Ok(Literal::Float(f)) }
            TokenKind::LitString(s) => { self.advance(); Ok(Literal::String(s)) }
            TokenKind::LitTrue      => { self.advance(); Ok(Literal::Bool(true)) }
            TokenKind::LitFalse     => { self.advance(); Ok(Literal::Bool(false)) }
            TokenKind::LitNull      => { self.advance(); Ok(Literal::Null) }
            other => Err(ParseError::new(
                format!("expected literal, found {:?}", other),
                span,
            )),
        }
    }
}
