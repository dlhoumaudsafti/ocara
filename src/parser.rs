use crate::ast::*;
use crate::token::{Span, Token, TokenKind};

// ─────────────────────────────────────────────────────────────────────────────
// Erreur de parsing
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span:    Span,
}

impl ParseError {
    fn new(msg: impl Into<String>, span: Span) -> Self {
        Self { message: msg.into(), span }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.span, self.message)
    }
}

impl std::error::Error for ParseError {}

pub type ParseResult<T> = Result<T, ParseError>;

// ─────────────────────────────────────────────────────────────────────────────
// Parser
// ─────────────────────────────────────────────────────────────────────────────

pub struct Parser {
    tokens: Vec<Token>,
    pos:    usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    // ── Primitives ───────────────────────────────────────────────────────────

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn span(&self) -> Span {
        self.current().span.clone()
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if tok.kind != TokenKind::Eof {
            self.pos += 1;
        }
        tok
    }

    fn check_exact(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == kind
    }

    fn eat(&mut self, kind: &TokenKind) -> ParseResult<Token> {
        if self.check_exact(kind) {
            Ok(self.advance().clone())
        } else {
            Err(ParseError::new(
                format!("attendu {:?}, trouvé {:?}", kind, self.peek_kind()),
                self.span(),
            ))
        }
    }

    fn eat_ident(&mut self) -> ParseResult<(String, Span)> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok((name, span))
            }
            other => Err(ParseError::new(
                format!("attendu un identifiant, trouvé {:?}", other),
                span,
            )),
        }
    }

    // ── Point d'entrée ───────────────────────────────────────────────────────

    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let mut program = Program::new();

        while !self.check_exact(&TokenKind::Eof) {
            match self.peek_kind().clone() {
                TokenKind::Import => {
                    program.imports.push(self.parse_import()?);
                }
                TokenKind::Const => {
                    program.consts.push(self.parse_const_decl()?);
                }
                TokenKind::Module => {
                    program.modules.push(self.parse_module()?);
                }
                TokenKind::Class => {
                    program.classes.push(self.parse_class()?);
                }
                TokenKind::Interface => {
                    program.interfaces.push(self.parse_interface()?);
                }
                TokenKind::Function => {
                    program.functions.push(self.parse_func()?);
                }
                other => {
                    return Err(ParseError::new(
                        format!("déclaration de haut niveau inattendue : {:?}", other),
                        self.span(),
                    ))
                }
            }
        }

        Ok(program)
    }

    // ── Import ───────────────────────────────────────────────────────────────

    fn parse_import(&mut self) -> ParseResult<ImportDecl> {
        let span = self.span();
        self.eat(&TokenKind::Import)?;

        let mut path = vec![self.eat_ident()?.0];
        while self.check_exact(&TokenKind::Dot) {
            self.advance(); // '.'
            // Accepter `*` (Star) comme dernier segment : import ocara.*
            if self.check_exact(&TokenKind::Star) {
                self.advance();
                path.push("*".to_string());
                break;
            }
            path.push(self.eat_ident()?.0);
        }

        let alias = if self.check_exact(&TokenKind::As) {
            self.advance();
            Some(self.eat_ident()?.0)
        } else {
            None
        };

        Ok(ImportDecl { path, alias, span })
    }

    // ── Constante globale ────────────────────────────────────────────────────

    fn parse_const_decl(&mut self) -> ParseResult<ConstDecl> {
        let span = self.span();
        self.eat(&TokenKind::Const)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.eat(&TokenKind::Eq)?;
        let value = self.parse_expr()?;
        Ok(ConstDecl { name, ty, value, span })
    }

    // ── Types ────────────────────────────────────────────────────────────────

    fn parse_type(&mut self) -> ParseResult<Type> {
        let first = self.parse_type_base()?;

        // Type union : `T | U | ...`
        if self.check_exact(&TokenKind::Pipe) {
            let mut variants = vec![first];
            while self.check_exact(&TokenKind::Pipe) {
                self.advance();
                variants.push(self.parse_type_base()?);
            }
            return Ok(Type::Union(variants));
        }

        Ok(first)
    }

    /// Analyse un type de base sans consommer les `|` qui suivent (appelé depuis parse_type pour les unions).
    fn parse_type_base(&mut self) -> ParseResult<Type> {
        let base = match self.peek_kind().clone() {
            TokenKind::TInt    => { self.advance(); Type::Int    }
            TokenKind::TFloat  => { self.advance(); Type::Float  }
            TokenKind::TString => { self.advance(); Type::String }
            TokenKind::TBool   => { self.advance(); Type::Bool   }
            TokenKind::TMixed    => { self.advance(); Type::Mixed    }
            TokenKind::TVoid   => { self.advance(); Type::Void   }
            TokenKind::LitNull => { self.advance(); Type::Null   }

            TokenKind::TMap => {
                self.advance();
                self.eat(&TokenKind::Lt)?;
                let k = self.parse_type()?;
                self.eat(&TokenKind::Comma)?;
                let v = self.parse_type()?;
                self.eat(&TokenKind::Gt)?;
                Type::Map(Box::new(k), Box::new(v))
            }

            TokenKind::Ident(name) => {
                self.advance();
                // `Function<ReturnType>` est un type de première classe avec retour typé
                if name == "Function" {
                    self.eat(&TokenKind::Lt)?;
                    let ret_ty = self.parse_type()?;
                    self.eat(&TokenKind::Gt)?;
                    return Ok(Type::Function(Box::new(ret_ty)));
                }
                if self.check_exact(&TokenKind::Dot) {
                    let mut parts = vec![name];
                    while self.check_exact(&TokenKind::Dot) {
                        self.advance();
                        parts.push(self.eat_ident()?.0);
                    }
                    Type::Qualified(parts)
                } else {
                    Type::Named(name)
                }
            }

            other => {
                return Err(ParseError::new(
                    format!("type attendu, trouvé {:?}", other),
                    self.span(),
                ))
            }
        };

        let mut ty = base;
        while self.check_exact(&TokenKind::LBracket) {
            self.advance();
            self.eat(&TokenKind::RBracket)?;
            ty = Type::Array(Box::new(ty));
        }
        Ok(ty)
    }

    // ── Paramètres ───────────────────────────────────────────────────────────

    fn parse_param_list(&mut self) -> ParseResult<Vec<Param>> {
        let mut params = Vec::new();
        if self.check_exact(&TokenKind::RParen) {
            return Ok(params);
        }
        params.push(self.parse_param()?);
        while self.check_exact(&TokenKind::Comma) {
            self.advance();
            params.push(self.parse_param()?);
        }
        Ok(params)
    }

    fn parse_param(&mut self) -> ParseResult<Param> {
        let span = self.span();
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        Ok(Param { name, ty, span })
    }

    // ── Fonction ─────────────────────────────────────────────────────────────

    fn parse_func(&mut self) -> ParseResult<FuncDecl> {
        let span = self.span();
        self.eat(&TokenKind::Function)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.eat(&TokenKind::RParen)?;
        self.eat(&TokenKind::Colon)?;
        let ret_ty = self.parse_type()?;
        let body = self.parse_block()?;
        Ok(FuncDecl { name, params, ret_ty, body, span })
    }

    fn parse_method_decl(&mut self) -> ParseResult<FuncDecl> {
        let span = self.span();
        self.eat(&TokenKind::Method)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.eat(&TokenKind::RParen)?;
        self.eat(&TokenKind::Colon)?;
        let ret_ty = self.parse_type()?;
        let body = self.parse_block()?;
        Ok(FuncDecl { name, params, ret_ty, body, span })
    }

    // ── Classe ───────────────────────────────────────────────────────────────

    fn parse_class(&mut self) -> ParseResult<ClassDecl> {
        let span = self.span();
        self.eat(&TokenKind::Class)?;
        let (name, _) = self.eat_ident()?;

        let extends = if self.check_exact(&TokenKind::Extends) {
            self.advance();
            Some(self.eat_ident()?.0)
        } else {
            None
        };

        let mut modules = Vec::new();
        if self.check_exact(&TokenKind::Modules) {
            self.advance();
            modules.push(self.eat_ident()?.0);
            while self.check_exact(&TokenKind::Comma) {
                self.advance();
                modules.push(self.eat_ident()?.0);
            }
        }

        let mut implements = Vec::new();
        if self.check_exact(&TokenKind::Implements) {
            self.advance();
            implements.push(self.eat_ident()?.0);
            while self.check_exact(&TokenKind::Comma) {
                self.advance();
                implements.push(self.eat_ident()?.0);
            }
        }

        self.eat(&TokenKind::LBrace)?;
        let mut members = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            members.push(self.parse_class_member()?);
        }
        self.eat(&TokenKind::RBrace)?;

        Ok(ClassDecl { name, extends, modules, implements, members, span })
    }

    // ── Module (mixin) ───────────────────────────────────────────────────────

    fn parse_module(&mut self) -> ParseResult<ModuleDecl> {
        let span = self.span();
        self.eat(&TokenKind::Module)?;
        let (name, _) = self.eat_ident()?;

        self.eat(&TokenKind::LBrace)?;
        let mut members = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            members.push(self.parse_class_member()?);
        }
        self.eat(&TokenKind::RBrace)?;

        Ok(ModuleDecl { name, members, span })
    }

    fn parse_class_member(&mut self) -> ParseResult<ClassMember> {
        let span = self.span();

        // Constructeur : `init(...) { }`
        if self.check_exact(&TokenKind::Init) {
            self.advance();
            self.eat(&TokenKind::LParen)?;
            let params = self.parse_param_list()?;
            self.eat(&TokenKind::RParen)?;
            let body = self.parse_block()?;
            return Ok(ClassMember::Constructor { params, body, span });
        }

        // Visibilité obligatoire en premier : `public|private|protected`
        let vis = self.parse_visibility()?;

        // Modificateur statique optionnel après la visibilité : `public static method ...`
        let is_static = if self.check_exact(&TokenKind::Static) {
            self.advance();
            true
        } else {
            false
        };

        // `static` n'est autorisé que sur les méthodes
        if is_static && !self.check_exact(&TokenKind::Method) {
            return Err(ParseError::new(
                "'static' n'est autorisé que sur les méthodes (method)",
                self.span(),
            ));
        }

        // Constante de classe : `public const NAME:T = value`
        if self.check_exact(&TokenKind::Const) {
            self.advance();
            let (name, _) = self.eat_ident()?;
            self.eat(&TokenKind::Colon)?;
            let ty = self.parse_type()?;
            self.eat(&TokenKind::Eq)?;
            let value = self.parse_expr()?;
            return Ok(ClassMember::Const { vis, name, ty, value, span });
        }

        // Méthode : `public [static] method foo(...): T { }`
        if self.check_exact(&TokenKind::Method) {
            let decl = self.parse_method_decl()?;
            return Ok(ClassMember::Method { vis, is_static, decl, span });
        }

        // Champ : `vis property name:T` — seul 'property' est autorisé pour un champ de classe
        let _mutable = match self.peek_kind() {
            TokenKind::Property => { self.advance(); true }
            TokenKind::Var => {
                return Err(ParseError::new(
                    "'var' est interdit sur un champ de classe : utilisez 'property'".to_string(),
                    self.span(),
                ));
            }
            TokenKind::Scoped => {
                return Err(ParseError::new(
                    "'scoped' est interdit sur un champ de classe : un champ vit aussi longtemps que l'objet, utilisez 'property'".to_string(),
                    self.span(),
                ));
            }
            other => return Err(ParseError::new(
                format!("'property', 'const' ou 'method' attendu, trouvé {:?}", other),
                self.span(),
            )),
        };
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        Ok(ClassMember::Field { vis, mutable: true, name, ty, span })
    }

    fn parse_visibility(&mut self) -> ParseResult<Visibility> {
        let span = self.span();
        match self.peek_kind() {
            TokenKind::Public    => { self.advance(); Ok(Visibility::Public)    }
            TokenKind::Private   => { self.advance(); Ok(Visibility::Private)   }
            TokenKind::Protected => { self.advance(); Ok(Visibility::Protected) }
            other => Err(ParseError::new(
                format!("visibilité (public/private/protected) attendue, trouvé {:?}", other),
                span,
            )),
        }
    }

    // ── Interface ────────────────────────────────────────────────────────────

    fn parse_interface(&mut self) -> ParseResult<InterfaceDecl> {
        let span = self.span();
        self.eat(&TokenKind::Interface)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::LBrace)?;

        let mut methods = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            methods.push(self.parse_interface_method()?);
        }
        self.eat(&TokenKind::RBrace)?;

        Ok(InterfaceDecl { name, methods, span })
    }

    fn parse_interface_method(&mut self) -> ParseResult<InterfaceMethod> {
        let span = self.span();
        self.eat(&TokenKind::Method)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.eat(&TokenKind::RParen)?;
        self.eat(&TokenKind::Colon)?;
        let ret_ty = self.parse_type()?;
        Ok(InterfaceMethod { name, params, ret_ty, span })
    }

    // ── Block & Statements ───────────────────────────────────────────────────

    fn parse_block(&mut self) -> ParseResult<Block> {
        let span = self.span();
        self.eat(&TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            stmts.push(self.parse_stmt()?);
        }
        self.eat(&TokenKind::RBrace)?;
        Ok(Block { stmts, span })
    }

    fn parse_stmt(&mut self) -> ParseResult<Stmt> {
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
            handlers.push(crate::ast::OnClause { binding, class_filter, body: handler_body, span: h_span });
        }

        if handlers.is_empty() {
            return Err(crate::parser::ParseError {
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

    // ── Expressions ──────────────────────────────────────────────────────────
    // Précédence (croissante) :
    //   Or → And → Equality → Comparison → Additive → Multiplicative → Unary → Postfix → Primary

    pub fn parse_expr(&mut self) -> ParseResult<Expr> {
        self.parse_or()
    }

    /// Parse expression sans autoriser 'is' au top level
    /// Utilisé pour les body de match arms où 'is' est réservé aux patterns
    fn parse_expr_no_is(&mut self) -> ParseResult<Expr> {
        self.parse_or_no_is()
    }

    fn parse_or(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_and()?;
        while self.check_exact(&TokenKind::KwOr) {
            let span = self.span();
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    /// Version sans 'is' pour les match arms
    fn parse_or_no_is(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_and_no_is()?;
        while self.check_exact(&TokenKind::KwOr) {
            let span = self.span();
            self.advance();
            let right = self.parse_and_no_is()?;
            left = Expr::Binary {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_equality()?;
        while self.check_exact(&TokenKind::KwAnd) {
            let span = self.span();
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    /// Version sans 'is' pour les match arms
    fn parse_and_no_is(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_equality_no_is()?;
        while self.check_exact(&TokenKind::KwAnd) {
            let span = self.span();
            self.advance();
            let right = self.parse_equality_no_is()?;
            left = Expr::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_is_check()?;
        loop {
            let span = self.span();
            let op = match self.peek_kind() {
                TokenKind::EqEq   => BinOp::EqEq,
                TokenKind::BangEq => BinOp::NotEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_is_check()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    /// Version sans 'is' pour les match arms
    fn parse_equality_no_is(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let span = self.span();
            let op = match self.peek_kind() {
                TokenKind::EqEq   => BinOp::EqEq,
                TokenKind::BangEq => BinOp::NotEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    /// Test de type : `expr is Type` — retourne bool
    fn parse_is_check(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_comparison()?;
        if self.check_exact(&TokenKind::Is) {
            let span = self.span();
            self.advance();
            let ty = self.parse_type()?;
            expr = Expr::IsCheck {
                expr: Box::new(expr),
                ty,
                span,
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_range()?;
        loop {
            let span = self.span();
            let op = match self.peek_kind() {
                TokenKind::Lt   => BinOp::Lt,
                TokenKind::LtEq => BinOp::LtEq,
                TokenKind::Gt   => BinOp::Gt,
                TokenKind::GtEq => BinOp::GtEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    /// Range : `expr..expr`  — précédence entre comparaison et addition
    fn parse_range(&mut self) -> ParseResult<Expr> {
        let left = self.parse_additive()?;
        if self.check_exact(&TokenKind::DotDot) {
            let span = self.span();
            self.advance();
            let right = self.parse_additive()?;
            return Ok(Expr::Range {
                start: Box::new(left),
                end:   Box::new(right),
                span,
            });
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let span = self.span();
            let op = match self.peek_kind() {
                TokenKind::Plus  => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let span = self.span();
            let op = match self.peek_kind() {
                TokenKind::Star    => BinOp::Mul,
                TokenKind::Slash   => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Expr> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::KwNot => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary { op: UnaryOp::Not, operand: Box::new(operand), span })
            }
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary { op: UnaryOp::Neg, operand: Box::new(operand), span })
            }
            _ => self.parse_postfix(),
        }
    }

    /// Postfix : appels de méthode / accès de champ / accès statique
    fn parse_postfix(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            let span = self.span();
            match self.peek_kind() {
                // accès de champ ou appel de méthode : `expr.ident` / `expr.ident(...)`
                TokenKind::Dot => {
                    self.advance();
                    let (field, _) = self.eat_ident()?;

                    if self.check_exact(&TokenKind::LParen) {
                        // appel de méthode
                        self.advance();
                        let args = self.parse_arg_list()?;
                        self.eat(&TokenKind::RParen)?;
                        expr = Expr::Call {
                            callee: Box::new(Expr::Field {
                                object: Box::new(expr),
                                field,
                                span: span.clone(),
                            }),
                            args,
                            span,
                        };
                    } else {
                        // accès simple
                        // Annotation de type optionnelle après `.field` (spec: `user.age:int`)
                        // On la consomme silencieusement (usage dans match/switch)
                        if self.check_exact(&TokenKind::Colon) {
                            self.advance();
                            self.parse_type()?; // type hint ignoré à ce stade
                        }
                        expr = Expr::Field {
                            object: Box::new(expr),
                            field,
                            span,
                        };
                    }
                }

                // appel direct : `expr(...)`
                TokenKind::LParen => {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.eat(&TokenKind::RParen)?;
                    expr = Expr::Call { callee: Box::new(expr), args, span };
                }

                // accès par index : `expr[index]`
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.eat(&TokenKind::RBracket)?;
                    expr = Expr::Index {
                        object: Box::new(expr),
                        index:  Box::new(index),
                        span,
                    };
                }

                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expr> {
        let span = self.span();

        match self.peek_kind().clone() {
            // ── Littéraux ───────────────────────────────────────────────────
            TokenKind::LitInt(n) => {
                self.advance();
                Ok(Expr::Literal(Literal::Int(n), span))
            }
            TokenKind::LitFloat(f) => {
                self.advance();
                Ok(Expr::Literal(Literal::Float(f), span))
            }
            TokenKind::LitString(s) => {
                self.advance();
                Ok(Expr::Literal(Literal::String(s), span))
            }
            TokenKind::LitTemplate(raw_parts) => {
                self.advance();
                let mut parts = Vec::new();
                for part in raw_parts {
                    match part {
                        crate::token::TemplatePart::Literal(s) => {
                            parts.push(TemplatePartExpr::Literal(s));
                        }
                        crate::token::TemplatePart::ExprSrc(src) => {
                            // Re-parse l'expression depuis le source brut
                            let mut sub_lex = crate::lexer::Lexer::new(&src);
                            let sub_tokens = sub_lex.tokenize().map_err(|e| {
                                ParseError::new(
                                    format!("erreur dans interpolation `${{{}}}` : {}", src, e),
                                    span.clone(),
                                )
                            })?;
                            let mut sub_parser = Parser::new(sub_tokens);
                            let expr = sub_parser.parse_expr()?;
                            parts.push(TemplatePartExpr::Expr(Box::new(expr)));
                        }
                    }
                }
                Ok(Expr::Template { parts, span })
            }
            TokenKind::LitTrue => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true), span))
            }
            TokenKind::LitFalse => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false), span))
            }
            TokenKind::LitNull => {
                self.advance();
                Ok(Expr::Literal(Literal::Null, span))
            }

            // ── nameless (closure anonyme) ───────────────────────────────────
            TokenKind::KwNameless => {
                self.advance();
                self.eat(&TokenKind::LParen)?;
                let params = self.parse_param_list()?;
                self.eat(&TokenKind::RParen)?;
                let ret_ty = if self.check_exact(&TokenKind::Colon) {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let body = self.parse_block()?;
                Ok(Expr::Nameless { params, ret_ty, body, span })
            }

            // ── self ────────────────────────────────────────────────────────
            TokenKind::SelfKw => {                self.advance();

                // self::method(args) — appel d'une méthode statique de la classe courante
                if self.check_exact(&TokenKind::ColonColon) {
                    self.advance();
                    let (member, _) = self.eat_ident()?;
                    if self.check_exact(&TokenKind::LParen) {
                        self.advance();
                        let args = self.parse_arg_list()?;
                        self.eat(&TokenKind::RParen)?;
                        // "<self>" est un marqueur résolu plus tard par le typecheck/lower
                        return Ok(Expr::StaticCall { class: "<self>".into(), method: member, args, span });
                    }
                    // self::CONST — constante de classe courante
                    return Ok(Expr::StaticConst { class: "<self>".into(), name: member, span });
                }

                Ok(Expr::SelfExpr(span))
            }

            // ── use (instanciation) ─────────────────────────────────────────
            TokenKind::Use => {
                self.advance();
                let (class, _) = self.eat_ident()?;
                self.eat(&TokenKind::LParen)?;
                let args = self.parse_arg_list()?;
                self.eat(&TokenKind::RParen)?;
                Ok(Expr::New { class, args, span })
            }

            // ── match ───────────────────────────────────────────────────────
            TokenKind::Match => self.parse_match_expr(),

            // ── map littéral { "clé": valeur, ... } ─────────────────────────
            TokenKind::LBrace => {
                self.advance();
                let mut entries = Vec::new();
                while !self.check_exact(&TokenKind::RBrace) {
                    let k = self.parse_expr()?;
                    self.eat(&TokenKind::Colon)?;
                    let v = self.parse_expr()?;
                    entries.push((k, v));
                    if self.check_exact(&TokenKind::Comma) {
                        self.advance();
                    }
                }
                self.eat(&TokenKind::RBrace)?;
                Ok(Expr::Map { entries, span })
            }

            // ── tableau littéral [ elem, ... ] ───────────────────────────────
            TokenKind::LBracket => {
                self.advance();
                // Tableau vide
                if self.check_exact(&TokenKind::RBracket) {
                    self.advance();
                    return Ok(Expr::Array { elements: Vec::new(), span });
                }
                let first = self.parse_expr()?;
                let mut elements = vec![first];
                while self.check_exact(&TokenKind::Comma) {
                    self.advance();
                    if self.check_exact(&TokenKind::RBracket) { break; }
                    elements.push(self.parse_expr()?);
                }
                self.eat(&TokenKind::RBracket)?;
                Ok(Expr::Array { elements, span })
            }

            // ── expression parenthésée ──────────────────────────────────────
            TokenKind::LParen => {
                self.advance();
                let inner = self.parse_expr()?;
                self.eat(&TokenKind::RParen)?;
                Ok(inner)
            }

            // ── Identifiant ou accès statique `A::b(...)` / `A::NAME` ──────────────
            TokenKind::Ident(name) => {
                self.advance();

                // Accès statique : `Class::method(...)` ou `Class::NAME`
                if self.check_exact(&TokenKind::ColonColon) {
                    self.advance();
                    let (member, _) = self.eat_ident()?;
                    if self.check_exact(&TokenKind::LParen) {
                        // Appel de méthode statique
                        self.advance();
                        let args = self.parse_arg_list()?;
                        self.eat(&TokenKind::RParen)?;
                        return Ok(Expr::StaticCall { class: name, method: member, args, span });
                    } else {
                        // Lecture d'une constante de classe
                        return Ok(Expr::StaticConst { class: name, name: member, span });
                    }
                }

                Ok(Expr::Ident(name, span))
            }

            _ => Err(ParseError::new(
                format!("expression primaire inattendue : {:?}", self.peek_kind()),
                span,
            )),
        }
    }

    // ── Match expression ─────────────────────────────────────────────────────

    fn parse_match_expr(&mut self) -> ParseResult<Expr> {
        let span = self.span();
        self.eat(&TokenKind::Match)?;
        let subject = self.parse_postfix()?; // `user.age:int` → postfix gère l'annotation
        // Annotation de type optionnelle directement sur le sujet : `match expr:Type {`
        if self.check_exact(&TokenKind::Colon) {
            self.advance();
            self.parse_type()?; // consomme et ignore le hint
        }
        self.eat(&TokenKind::LBrace)?;

        let mut arms = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            let arm_span = self.span();
            if self.check_exact(&TokenKind::Default) {
                self.advance();
                self.eat(&TokenKind::Arrow)?;
                let body = self.parse_expr_no_is()?;
                arms.push(MatchArm { pattern: None, body, span: arm_span });
            } else {
                let pattern = Some(self.parse_match_pattern()?);
                self.eat(&TokenKind::Arrow)?;
                let body = self.parse_expr_no_is()?;
                arms.push(MatchArm { pattern, body, span: arm_span });
            }
        }

        self.eat(&TokenKind::RBrace)?;
        Ok(Expr::Match { subject: Box::new(subject), arms, span })
    }

    // ── Pattern de match ─────────────────────────────────────────────────────

    fn parse_match_pattern(&mut self) -> ParseResult<MatchPattern> {
        if self.check_exact(&TokenKind::Is) {
            // Pattern : `is Type`
            self.advance();
            let ty = self.parse_type()?;
            Ok(MatchPattern::IsType(ty))
        } else {
            // Pattern : littéral
            let lit = self.parse_literal()?;
            Ok(MatchPattern::Literal(lit))
        }
    }

    // ── Littéral isolé (pour patterns) ───────────────────────────────────────

    fn parse_literal(&mut self) -> ParseResult<Literal> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::LitInt(n)    => { self.advance(); Ok(Literal::Int(n)) }
            TokenKind::LitFloat(f)  => { self.advance(); Ok(Literal::Float(f)) }
            TokenKind::LitString(s) => { self.advance(); Ok(Literal::String(s)) }
            TokenKind::LitTrue      => { self.advance(); Ok(Literal::Bool(true)) }
            TokenKind::LitFalse     => { self.advance(); Ok(Literal::Bool(false)) }
            TokenKind::LitNull      => { self.advance(); Ok(Literal::Null) }
            other => Err(ParseError::new(
                format!("littéral attendu, trouvé {:?}", other),
                span,
            )),
        }
    }

    // ── Liste d'arguments ────────────────────────────────────────────────────

    fn parse_arg_list(&mut self) -> ParseResult<Vec<Expr>> {
        let mut args = Vec::new();
        if self.check_exact(&TokenKind::RParen) {
            return Ok(args);
        }
        args.push(self.parse_expr()?);
        while self.check_exact(&TokenKind::Comma) {
            self.advance();
            if self.check_exact(&TokenKind::RParen) { break; }
            args.push(self.parse_expr()?);
        }
        Ok(args)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().expect("lex error");
        Parser::new(tokens).parse_program().expect("parse error")
    }

    fn parse_expr(src: &str) -> Expr {
        let tokens = Lexer::new(src).tokenize().expect("lex error");
        Parser::new(tokens).parse_expr().expect("parse error")
    }

    // ── Import ───────────────────────────────────────────────────────────────

    #[test]
    fn test_import_simple() {
        let p = parse("import datas.User");
        assert_eq!(p.imports[0].path, vec!["datas", "User"]);
        assert_eq!(p.imports[0].alias, None);
    }

    #[test]
    fn test_import_alias() {
        let p = parse("import datas.User as UserData");
        assert_eq!(p.imports[0].alias, Some("UserData".into()));
    }

    // ── Constante ────────────────────────────────────────────────────────────

    #[test]
    fn test_const() {
        let p = parse("const TAX:float = 0.2");
        assert_eq!(p.consts[0].name, "TAX");
        assert_eq!(p.consts[0].ty, Type::Float);
        assert!(matches!(p.consts[0].value, Expr::Literal(Literal::Float(_), _)));
    }

    // ── Fonction ─────────────────────────────────────────────────────────────

    #[test]
    fn test_func_empty() {
        let p = parse("function main(): int { return 0 }");
        assert_eq!(p.functions[0].name, "main");
        assert_eq!(p.functions[0].ret_ty, Type::Int);
        assert_eq!(p.functions[0].params.len(), 0);
    }

    #[test]
    fn test_func_with_params() {
        let p = parse("function add(a:int, b:int): int { return 0 }");
        let params = &p.functions[0].params;
        assert_eq!(params[0].name, "a");
        assert_eq!(params[0].ty, Type::Int);
        assert_eq!(params[1].name, "b");
    }

    // ── Classe ───────────────────────────────────────────────────────────────

    #[test]
    fn test_class_basic() {
        let p = parse(r#"
            class User {
                private property name:string
                init(name:string) {}
                public method greet(): void {}
            }
        "#);
        let cls = &p.classes[0];
        assert_eq!(cls.name, "User");
        assert_eq!(cls.members.len(), 3);
    }

    #[test]
    fn test_class_extends_implements() {
        let p = parse("class A extends B implements C, D {}");
        let cls = &p.classes[0];
        assert_eq!(cls.extends, Some("B".into()));
        assert_eq!(cls.implements, vec!["C", "D"]);
    }

    // ── Interface ────────────────────────────────────────────────────────────

    #[test]
    fn test_interface() {
        let p = parse("interface Logger { method log(msg:string): void }");
        assert_eq!(p.interfaces[0].name, "Logger");
        assert_eq!(p.interfaces[0].methods[0].name, "log");
    }

    // ── Expressions ──────────────────────────────────────────────────────────

    #[test]
    fn test_expr_binary() {
        let expr = parse_expr("1 + 2 * 3");
        // doit respecter la précédence : 1 + (2 * 3)
        if let Expr::Binary { op: BinOp::Add, right, .. } = &expr {
            assert!(matches!(right.as_ref(), Expr::Binary { op: BinOp::Mul, .. }));
        } else {
            panic!("précédence incorrecte");
        }
    }

    #[test]
    fn test_expr_field_access() {
        let expr = parse_expr("user.age");
        assert!(matches!(expr, Expr::Field { .. }));
    }

    #[test]
    fn test_expr_range() {
        let expr = parse_expr("0..5");
        assert!(matches!(expr, Expr::Range { .. }));
    }

    #[test]
    fn test_expr_new() {
        let expr = parse_expr(r#"use User("Alice", 30)"#);
        if let Expr::New { class, args, .. } = expr {
            assert_eq!(class, "User");
            assert_eq!(args.len(), 2);
        } else {
            panic!("expected New");
        }
    }

    #[test]
    fn test_expr_static_call() {
        let expr = parse_expr("Math::abs(x)");
        assert!(matches!(expr, Expr::StaticCall { .. }));
    }

    // ── Types ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_type_array() {
        let p = parse("function foo(a:int[]): int {}");
        assert_eq!(p.functions[0].params[0].ty, Type::Array(Box::new(Type::Int)));
    }

    #[test]
    fn test_type_map() {
        let p = parse("function foo(m:map<string,int>): int {}");
        assert_eq!(
            p.functions[0].params[0].ty,
            Type::Map(Box::new(Type::String), Box::new(Type::Int))
        );
    }

    #[test]
    fn test_type_qualified() {
        let p = parse("function foo(u:repository.User): void {}");
        assert_eq!(
            p.functions[0].params[0].ty,
            Type::Qualified(vec!["repository".into(), "User".into()])
        );
    }

    // ── Tableaux & Maps ───────────────────────────────────────────────────────

    #[test]
    fn test_array_literal() {
        let expr = parse_expr("[1, 2, 3]");
        if let Expr::Array { elements, .. } = expr {
            assert_eq!(elements.len(), 3);
        } else {
            panic!("expected Array");
        }
    }

    #[test]
    fn test_array_multidimensional() {
        let expr = parse_expr("[[1, 2], [3, 4]]");
        if let Expr::Array { elements, .. } = expr {
            assert_eq!(elements.len(), 2);
            assert!(matches!(elements[0], Expr::Array { .. }));
        } else {
            panic!("expected nested Array");
        }
    }

    #[test]
    fn test_map_literal() {
        let expr = parse_expr(r#"{"name": "Lucas", "age": 24}"#);
        if let Expr::Map { entries, .. } = expr {
            assert_eq!(entries.len(), 2);
        } else {
            panic!("expected Map");
        }
    }

    #[test]
    fn test_index_access() {
        let expr = parse_expr("arr[0]");
        assert!(matches!(expr, Expr::Index { .. }));
    }

    #[test]
    fn test_map_index_access() {
        let expr = parse_expr(r#"user["name"]"#);
        assert!(matches!(expr, Expr::Index { .. }));
    }
}
