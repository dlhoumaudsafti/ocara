/// Parsing des expressions

use crate::parsing::ast::*;
use crate::parsing::token::TokenKind;
use super::types::{Parser, ParseError, ParseResult};

/// Profondeur maximale de l'arbre d'une expression : au-delà, les passes
/// récursives du compilateur déborderaient leur pile (voir `main.rs`).
const MAX_EXPR_DEPTH: usize = 50_000;

impl Parser {
    /// Un niveau de plus dans l'arbre de l'expression en cours.
    pub(super) fn deepen(&mut self) -> ParseResult<()> {
        self.expr_depth += 1;
        if self.expr_depth > MAX_EXPR_DEPTH {
            return Err(ParseError::new(
                &format!("expression trop profonde (plus de {} niveaux d'imbrication ou d'opérateurs enchaînés) : découpez-la en variables intermédiaires", MAX_EXPR_DEPTH),
                self.span(),
            ));
        }
        Ok(())
    }

    /// Analyse une sous-expression imbriquée ; sa profondeur ne compte plus
    /// une fois refermée.
    fn nested<T>(&mut self, parse: impl FnOnce(&mut Self) -> ParseResult<T>) -> ParseResult<T> {
        let saved = self.expr_depth;
        let result = self.deepen().and_then(|_| parse(self));
        self.expr_depth = saved;
        result
    }

    // ── Expressions ──────────────────────────────────────────────────────────
    // Précédence (croissante) :
    //   Cond (`then`, voir cond.rs) → Or → And → Equality → Comparison → Additive → Multiplicative → Unary → Postfix → Primary

    pub fn parse_expr(&mut self) -> ParseResult<Expr> {
        self.nested(|p| p.parse_cond(Self::parse_or))
    }

    /// Parse expression sans autoriser 'is' au top level
    /// Utilisé pour les body de match arms où 'is' est réservé aux patterns
    fn parse_expr_no_is(&mut self) -> ParseResult<Expr> {
        self.nested(|p| p.parse_cond(Self::parse_or_no_is))
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
            self.deepen()?;
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
            self.deepen()?;
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_comparison()?;
        while self.check_exact(&TokenKind::KwAnd) {
            let span = self.span();
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
            self.deepen()?;
        }
        Ok(left)
    }

    /// Version sans 'is' pour les match arms
    fn parse_and_no_is(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_comparison_no_is()?;
        while self.check_exact(&TokenKind::KwAnd) {
            let span = self.span();
            self.advance();
            let right = self.parse_comparison_no_is()?;
            left = Expr::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
            self.deepen()?;
        }
        Ok(left)
    }

    /// Comparaisons — un seul niveau de précédence, toutes en toutes lettres :
    ///   equal | not equal | smaller | greater | smaller or equal | greater or equal
    /// Aucun symbole (===, ==, <=, <==, <, >==, >=, >) n'est accepté ici ; les
    /// anciens tokens symboliques ne sont conservés dans le lexer que pour
    /// produire un message de migration explicite (voir `removed_operator_error`).
    fn parse_comparison(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_is_check()?;
        loop {
            if let Some(op) = self.try_consume_compound_comparison()? {
                let span = left.span().clone();
                let right = self.parse_is_check()?;
                let full_span = span.union(right.span());
                left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span: full_span };
                self.deepen()?;
                continue;
            }
            let span = left.span().clone();
            let op = match self.peek_kind() {
                TokenKind::KwEqual   => BinOp::Equal,
                TokenKind::KwSmaller => BinOp::Smaller,
                TokenKind::KwGreater => BinOp::Greater,
                _ => {
                    self.reject_removed_comparison_operator()?;
                    break;
                }
            };
            self.advance();
            let right = self.parse_is_check()?;
            let full_span = span.union(right.span());
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span: full_span };
            self.deepen()?;
        }
        Ok(left)
    }

    /// Version sans 'is' pour les match arms
    fn parse_comparison_no_is(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_range()?;
        loop {
            if let Some(op) = self.try_consume_compound_comparison()? {
                let span = left.span().clone();
                let right = self.parse_range()?;
                let full_span = span.union(right.span());
                left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span: full_span };
                self.deepen()?;
                continue;
            }
            let span = left.span().clone();
            let op = match self.peek_kind() {
                TokenKind::KwEqual   => BinOp::Equal,
                TokenKind::KwSmaller => BinOp::Smaller,
                TokenKind::KwGreater => BinOp::Greater,
                _ => {
                    self.reject_removed_comparison_operator()?;
                    break;
                }
            };
            self.advance();
            let right = self.parse_range()?;
            let full_span = span.union(right.span());
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span: full_span };
            self.deepen()?;
        }
        Ok(left)
    }

    /// Reconnaît les opérateurs composés à trois mots (`not equal`,
    /// `smaller or equal`, `greater or equal`) sans les confondre avec les
    /// mots-clés `not`/`or` utilisés ailleurs (unaire, logique) : la
    /// désambiguïsation se fait uniquement par position (en tête d'un
    /// opérande déjà complet), exactement comme pour `not equal` avant lui.
    fn try_consume_compound_comparison(&mut self) -> ParseResult<Option<BinOp>> {
        if self.check_exact(&TokenKind::KwNot)
            && self.peek_ahead(1).map(|t| &t.kind) == Some(&TokenKind::KwEqual)
        {
            self.advance(); // 'not'
            self.advance(); // 'equal'
            return Ok(Some(BinOp::NotEqual));
        }
        if self.check_exact(&TokenKind::KwSmaller)
            && self.peek_ahead(1).map(|t| &t.kind) == Some(&TokenKind::KwOr)
            && self.peek_ahead(2).map(|t| &t.kind) == Some(&TokenKind::KwEqual)
        {
            self.advance(); // 'smaller'
            self.advance(); // 'or'
            self.advance(); // 'equal'
            return Ok(Some(BinOp::SmallerOrEqual));
        }
        if self.check_exact(&TokenKind::KwGreater)
            && self.peek_ahead(1).map(|t| &t.kind) == Some(&TokenKind::KwOr)
            && self.peek_ahead(2).map(|t| &t.kind) == Some(&TokenKind::KwEqual)
        {
            self.advance(); // 'greater'
            self.advance(); // 'or'
            self.advance(); // 'equal'
            return Ok(Some(BinOp::GreaterOrEqual));
        }
        Ok(None)
    }

    /// Si le token courant est un ancien opérateur symbolique (`===`, `==`,
    /// `<=`, `<==`, `<`, `>==`, `>=`, `>`, `!=`, `!==`), renvoie une erreur de
    /// migration nommant explicitement le remplaçant littéral. Sinon, ne fait
    /// rien (le token n'est pas un opérateur de comparaison connu — laisse
    /// l'appelant `break` normalement, ce n'est pas forcément une erreur ici).
    fn reject_removed_comparison_operator(&mut self) -> ParseResult<()> {
        let (removed, replacement) = match self.peek_kind() {
            TokenKind::EqEq    => ("==",  "equal"),
            TokenKind::EqEqEq  => ("===", "equal"),
            TokenKind::BangEq  => ("!=",  "not equal"),
            TokenKind::BangEqEq => ("!==", "not equal"),
            TokenKind::LtEq    => ("<=",  "smaller or equal"),
            TokenKind::LtEqEq  => ("<==", "smaller or equal"),
            TokenKind::GtEq    => (">=",  "greater or equal"),
            TokenKind::GtEqEq  => (">==", "greater or equal"),
            TokenKind::Lt      => ("<",   "smaller"),
            TokenKind::Gt      => (">",   "greater"),
            _ => return Ok(()),
        };
        Err(ParseError::new(
            format!("operator '{}' has been removed — use '{}' instead", removed, replacement),
            self.span(),
        ))
    }

    /// Test de type : `expr is Type` — retourne bool
    fn parse_is_check(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_range()?;
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

    /// Plage : `a until b` (borne de fin exclue) ou `a to b` (incluse,
    /// réécrite `a until b + 1`) — précédence entre comparaison et
    /// addition. `to`/`until` ne sont des mots-clés qu'après une expression,
    /// sur la même ligne. L'ancienne forme `a..b` est refusée.
    fn parse_range(&mut self) -> ParseResult<Expr> {
        let left = self.parse_additive()?;
        if self.check_exact(&TokenKind::DotDot) {
            return Err(ParseError::new(
                "'..' n'existe plus : écrire 'a until b' (borne de fin exclue, ancien 'a..b') ou 'a to b' (borne de fin incluse)",
                self.span(),
            ));
        }
        let inclusive = match self.peek_kind() {
            TokenKind::Ident(w) if (w == "to" || w == "until") && self.previous_line() == Some(self.span().line) => w == "to",
            _ => return Ok(left),
        };
        let span = self.span();
        self.advance();
        let mut right = self.parse_additive()?;
        if inclusive {
            let one_span = right.span().clone();
            right = Expr::Binary {
                op: BinOp::Add,
                left: Box::new(right),
                right: Box::new(Expr::Literal(Literal::Int(1), one_span.clone())),
                span: one_span,
            };
        }
        Ok(Expr::Range { start: Box::new(left), end: Box::new(right), span })
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
            self.deepen()?;
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
            self.deepen()?;
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Expr> {
        self.nested(Self::parse_unary_operand)
    }

    fn parse_unary_operand(&mut self) -> ParseResult<Expr> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::KwNot => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary { op: UnaryOp::Not, operand: Box::new(operand), span })
            }
            // `-9223372036854775808` : INT_MIN, seul littéral dont la
            // magnitude dépasse `i64` (voir `int_literal`).
            TokenKind::Minus if self.peek_ahead(1).is_some_and(|t| t.kind == TokenKind::LitInt(i64::MIN)) => {
                self.advance();
                self.advance();
                Ok(Expr::Literal(Literal::Int(i64::MIN), span))
            }
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary { op: UnaryOp::Neg, operand: Box::new(operand), span })
            }
            TokenKind::Resolve => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Resolve { expr: Box::new(expr), span })
            }
            // Préfixe `++i`/`--i` — la forme de `target` (Ident/Field/Index)
            // est validée en sema, pas ici (même patron que `Stmt::Assign`).
            TokenKind::PlusPlus => {
                self.advance();
                let target = self.parse_unary()?;
                Ok(Expr::IncDec { op: IncDecOp::PreInc, target: Box::new(target), span })
            }
            TokenKind::MinusMinus => {
                self.advance();
                let target = self.parse_unary()?;
                Ok(Expr::IncDec { op: IncDecOp::PreDec, target: Box::new(target), span })
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
                    // `on` (mot-clé de `try … on`) reste un nom de membre après
                    // un point : `ui.on("action", Classe::methode)` (ocara.UI).
                    let field = if self.check_exact(&TokenKind::On) {
                        self.advance();
                        "on".to_string()
                    } else {
                        self.eat_ident()?.0
                    };

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
                        if self.check_exact(&TokenKind::Colon) && !self.in_map_key {
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

                // suffixe `expr++`/`expr--` — même remarque que le préfixe :
                // la forme de `expr` est validée en sema, pas ici. Restreint
                // à la MÊME ligne que la fin de `expr` (voir `previous_line`)
                // — sinon `expr` suivi, sur la ligne suivante, d'un `++i`/
                // `--i` préfixe (nouvelle instruction) se ferait avaler à
                // tort comme suffixe de CETTE expression.
                TokenKind::PlusPlus if self.previous_line() == Some(span.line) => {
                    self.advance();
                    expr = Expr::IncDec { op: IncDecOp::PostInc, target: Box::new(expr), span };
                }
                TokenKind::MinusMinus if self.previous_line() == Some(span.line) => {
                    self.advance();
                    expr = Expr::IncDec { op: IncDecOp::PostDec, target: Box::new(expr), span };
                }

                _ => break,
            }
            self.deepen()?;
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expr> {
        let span = self.span();

        match self.peek_kind().clone() {
            // ── Littéraux ───────────────────────────────────────────────────
            TokenKind::LitInt(n) => {
                let n = self.int_literal(n)?;
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
                        crate::parsing::token::TemplatePart::Literal(s) => {
                            parts.push(TemplatePartExpr::Literal(s));
                        }
                        crate::parsing::token::TemplatePart::ExprSrc(src, expr_start) => {
                            // Re-parse l'expression depuis le source brut —
                            // le sous-lexer/sous-parser ne connaissent que ce
                            // texte isolé, donc toute position qu'ils
                            // produisent démarre à (1,1) DE CE TEXTE, jamais
                            // la vraie position dans le fichier d'origine.
                            // `shift_expr_spans` corrige ça après coup à
                            // partir de `expr_start` (voir
                            // docs/roadmap.d/langage-template-interpolation-span-position.md).
                            let mut sub_lex = crate::parsing::lexer::Lexer::new(&src);
                            let sub_tokens = sub_lex.tokenize().map_err(|e| {
                                ParseError::new(
                                    format!("error in interpolation `${{{}}}`: {}", src, e),
                                    span.clone(),
                                )
                            })?;
                            let mut sub_parser = Parser::new(sub_tokens);
                            let mut expr = sub_parser.parse_expr()?;
                            crate::parsing::ast::shift_expr_spans(&mut expr, &expr_start);
                            // Noms de type de l'interpolation (`${Format::price(x)}`).
                            for (name, mut name_span) in std::mem::take(&mut sub_parser.type_refs) {
                                crate::parsing::ast::shift_span(&mut name_span, &expr_start);
                                self.type_refs.push((name, name_span));
                            }
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
            TokenKind::SelfKw => {
                self.advance();

                // self::method(args) — appel d'une méthode statique de la classe courante
                if self.check_exact(&TokenKind::ColonColon) {
                    self.advance();
                    let (member, _) = self.eat_ident()?;
                    if self.check_exact(&TokenKind::LParen) {
                        self.advance();
                        let args = self.parse_arg_list()?;
                        self.eat(&TokenKind::RParen)?;
                        // "<self>" est un marqueur résolu plus tard par le typecheck/lower
                        return Ok(Expr::StaticCall { class: "<self>".into(), method: member, args, span, type_args: Vec::new() });
                    }
                    // self::CONST — constante de classe courante
                    return Ok(Expr::StaticConst { class: "<self>".into(), name: member, span });
                }

                Ok(Expr::SelfExpr(span))
            }

            // ── parent ──────────────────────────────────────────────────────
            TokenKind::ParentKw => {
                self.advance();

                // parent::method(args) — appel d'une méthode statique de la classe parent
                if self.check_exact(&TokenKind::ColonColon) {
                    self.advance();
                    let (member, _) = self.eat_ident()?;
                    if self.check_exact(&TokenKind::LParen) {
                        self.advance();
                        let args = self.parse_arg_list()?;
                        self.eat(&TokenKind::RParen)?;
                        // "<parent>" est un marqueur résolu plus tard par le typecheck/lower
                        return Ok(Expr::StaticCall { class: "<parent>".into(), method: member, args, span, type_args: Vec::new() });
                    }
                    // parent::CONST — constante de classe parent
                    return Ok(Expr::StaticConst { class: "<parent>".into(), name: member, span });
                }

                Ok(Expr::ParentExpr(span))
            }

            // ── use (instanciation) ─────────────────────────────────────────
            TokenKind::Use => {
                self.advance();
                let (class, _) = self.eat_ident()?;
                
                // Arguments de type optionnels : use Cache<int, User>()
                let type_args = if self.check_exact(&TokenKind::Lt) {
                    self.advance();
                    let mut args = Vec::new();
                    args.push(self.parse_type()?);
                    while self.check_exact(&TokenKind::Comma) {
                        self.advance();
                        args.push(self.parse_type()?);
                    }
                    self.eat(&TokenKind::Gt)?;
                    args
                } else {
                    Vec::new()
                };
                
                self.eat(&TokenKind::LParen)?;
                let args = self.parse_arg_list()?;
                self.eat(&TokenKind::RParen)?;
                Ok(Expr::New { class, type_args, args, span })
            }

            // ── match ───────────────────────────────────────────────────────
            TokenKind::Match => self.parse_match_expr(),

            // ── map littéral { "clé": valeur, ... } ─────────────────────────
            TokenKind::LBrace => {
                self.advance();
                let mut entries = Vec::new();
                while !self.check_exact(&TokenKind::RBrace) {
                    let saved = std::mem::replace(&mut self.in_map_key, true);
                    let k = self.parse_expr();
                    self.in_map_key = saved;
                    let k = k?;
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

                // Appel statique sur un générique instancié : `Box<int>::of(4)`
                // (voir docs/roadmap.d/langage-generique-methode-statique.md).
                // Sinon (`a < b` n'existe plus), retour à l'erreur habituelle.
                let mut type_args = Vec::new();
                if self.check_exact(&TokenKind::Lt) {
                    let saved = self.pos;
                    match self.try_static_type_args() {
                        Some(args) => type_args = args,
                        None => self.pos = saved,
                    }
                }

                // Accès statique : `Class::method(...)` ou `Class::NAME`
                if self.check_exact(&TokenKind::ColonColon) {
                    self.advance();
                    // Classe telle qu'écrite (avant toute substitution `wiring`).
                    self.type_refs.push((name.clone(), span.clone()));
                    let (member, _) = self.eat_ident()?;
                    if self.check_exact(&TokenKind::LParen) {
                        // Appel de méthode statique
                        self.advance();
                        let args = self.parse_arg_list()?;
                        self.eat(&TokenKind::RParen)?;
                        return Ok(Expr::StaticCall { class: name, method: member, args, span, type_args });
                    } else {
                        // Lecture d'une constante de classe
                        return Ok(Expr::StaticConst { class: name, name: member, span });
                    }
                }

                Ok(Expr::Ident(name, span))
            }

            // ── Mots-clés runtime utilisables comme identifiants ────────────
            TokenKind::Main => {
                self.advance();
                Ok(Expr::Ident("main".to_string(), span))
            }
            TokenKind::Error => {
                self.advance();
                Ok(Expr::Ident("error".to_string(), span))
            }
            TokenKind::Success => {
                self.advance();
                Ok(Expr::Ident("success".to_string(), span))
            }
            TokenKind::Exit => {
                self.advance();
                Ok(Expr::Ident("exit".to_string(), span))
            }
            // "result" reste utilisable comme identifiant ordinaire en dehors d'un
            // bloc runtime (voir la même note dans eat_ident, primitives.rs) —
            // sans ce cas, une référence comme `return result` ou `result = ...`
            // échouerait avec "unexpected primary expression" partout où `result`
            // est un simple nom de variable (motif d'accumulateur très courant).
            TokenKind::Result => {
                self.advance();
                Ok(Expr::Ident("result".to_string(), span))
            }
            // "from" reste utilisable comme identifiant ordinaire hors de
            // `import X from Y` — voir la même note dans eat_ident (primitives.rs).
            TokenKind::From => {
                self.advance();
                Ok(Expr::Ident("from".to_string(), span))
            }
            // "message" reste utilisable comme identifiant ordinaire (ex:
            // `Exception.message`, `var message:string`, `return message`) —
            // le mot-clé `message` (type `message<T>`) n'a de sens qu'en
            // position de type. Voir la même note dans eat_ident (primitives.rs).
            TokenKind::TMessage => {
                self.advance();
                Ok(Expr::Ident("message".to_string(), span))
            }

            _ => Err(ParseError::new(
                format!("unexpected primary expression: {:?}", self.peek_kind()),
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

    // ── Liste d'arguments ────────────────────────────────────────────────────

    /// `<T, U>` suivi de `::` (appel statique sur un générique) : les
    /// arguments de type, ou `None` sans rien laisser derrière (l'appelant
    /// restaure la position).
    fn try_static_type_args(&mut self) -> Option<Vec<Type>> {
        let (refs, errors) = (self.type_refs.len(), self.errors.len());
        let parsed = (|| {
            self.eat(&TokenKind::Lt).ok()?;
            let mut args = vec![self.parse_type().ok()?];
            while self.check_exact(&TokenKind::Comma) {
                self.advance();
                args.push(self.parse_type().ok()?);
            }
            self.eat(&TokenKind::Gt).ok()?;
            self.check_exact(&TokenKind::ColonColon).then_some(args)
        })();
        if parsed.is_none() {
            self.type_refs.truncate(refs);
            self.errors.truncate(errors);
        }
        parsed
    }

    fn parse_arg_list(&mut self) -> ParseResult<Vec<Expr>> {
        let mut args = Vec::new();
        if self.check_exact(&TokenKind::RParen) {
            return Ok(args);
        }
        args.push(self.parse_arg()?);
        while self.check_exact(&TokenKind::Comma) {
            self.advance();
            if self.check_exact(&TokenKind::RParen) { break; }
            args.push(self.parse_arg()?);
        }
        Ok(args)
    }

    /// `Identifier ":" Expression` (argument nommé) ou `Expression` nue — un
    /// `ident` suivi d'un `:` simple n'a aucun autre sens en tête d'argument.
    fn parse_arg(&mut self) -> ParseResult<Expr> {
        let next_is_colon = self.peek_ahead(1).is_some_and(|t| t.kind == TokenKind::Colon);
        if next_is_colon {
            let (name, span) = self.eat_ident()?;
            self.eat(&TokenKind::Colon)?;
            let value = self.parse_expr()?;
            return Ok(Expr::NamedArg { name, value: Box::new(value), span });
        }
        self.parse_expr()
    }
}
