//! Clauses `when` : une déclaration (ou une instruction) précédée de
//! `when <condition>` n'existe que si la condition est vraie pour les
//! constantes de build courantes (`parsing::build`). Les variantes d'une
//! même déclaration sont choisies à la fin de leur conteneur : la première
//! clause vraie l'emporte, sinon la variante sans clause (ou `when default`).
//! Voir docs/roadmap.d/reflexion-declarations-conditionnelles-when.md.

use std::collections::HashMap;
use crate::parsing::ast::{ClassMember, Stmt};
use crate::parsing::build;
use crate::parsing::token::{Span, TokenKind};
use super::types::{ParseError, ParseResult, Parser};

#[derive(Debug, Clone)]
pub(super) enum Clause {
    /// Aucune clause : « sinon » implicite si la déclaration a des variantes.
    None,
    Default(Span),
    Cond(Cond, Span),
}

#[derive(Debug, Clone)]
pub(super) enum Cond {
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
    Cmp(Operand, CmpOp, Operand),
}

#[derive(Debug, Clone)]
pub(super) enum Operand {
    Const(String),
    Value(String),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum CmpOp { Is, IsNot, Greater, Smaller, GreaterEq, SmallerEq }

/// Variante d'un conteneur : clé de regroupement (genre + nom) et clause.
pub(super) struct Variant {
    pub key: String,
    pub clause: Clause,
    /// Position de la déclaration (diagnostic d'une variante sans clause).
    pub span: Span,
    /// Dernière ligne de la déclaration (grisage d'une variante écartée).
    pub end_line: usize,
}

impl Parser {
    /// Lignes `when …` avant une déclaration ; plusieurs lignes = `and`.
    pub(super) fn parse_when_clause(&mut self) -> ParseResult<Clause> {
        let mut clause = Clause::None;
        while self.check_exact(&TokenKind::When) {
            let span = self.span();
            self.advance();
            if self.check_exact(&TokenKind::Default) {
                self.advance();
                if !matches!(clause, Clause::None) {
                    return Err(ParseError::new("'when default' ne se combine pas avec une autre clause 'when'", span));
                }
                clause = Clause::Default(span);
                continue;
            }
            let cond = self.parse_when_or()?;
            clause = match clause {
                Clause::None => Clause::Cond(cond, span),
                Clause::Cond(prev, first) => Clause::Cond(Cond::And(Box::new(prev), Box::new(cond)), first),
                Clause::Default(_) => return Err(ParseError::new("'when default' ne se combine pas avec une autre clause 'when'", span)),
            };
        }
        Ok(clause)
    }

    fn parse_when_or(&mut self) -> ParseResult<Cond> {
        let mut cond = self.parse_when_and()?;
        while self.check_exact(&TokenKind::KwOr) {
            self.advance();
            cond = Cond::Or(Box::new(cond), Box::new(self.parse_when_and()?));
        }
        Ok(cond)
    }

    fn parse_when_and(&mut self) -> ParseResult<Cond> {
        let mut cond = self.parse_when_atom()?;
        while self.check_exact(&TokenKind::KwAnd) {
            self.advance();
            cond = Cond::And(Box::new(cond), Box::new(self.parse_when_atom()?));
        }
        Ok(cond)
    }

    fn parse_when_atom(&mut self) -> ParseResult<Cond> {
        if self.check_exact(&TokenKind::LParen) {
            self.advance();
            let cond = self.parse_when_or()?;
            self.eat(&TokenKind::RParen)?;
            return Ok(cond);
        }
        let lhs = self.parse_when_operand()?;
        let op_span = self.span();
        self.eat(&TokenKind::Is).map_err(|_| ParseError::new("clause 'when' : opérateur attendu ('is', 'is not', 'is greater', 'is smaller', 'is greater or equal', 'is smaller or equal')", op_span))?;
        let op = match self.peek_kind() {
            TokenKind::KwNot => { self.advance(); CmpOp::IsNot }
            TokenKind::KwGreater | TokenKind::KwSmaller => {
                let greater = self.check_exact(&TokenKind::KwGreater);
                self.advance();
                let or_equal = self.check_exact(&TokenKind::KwOr) && self.peek_ahead(1).is_some_and(|t| t.kind == TokenKind::KwEqual);
                if or_equal {
                    self.advance();
                    self.advance();
                }
                match (greater, or_equal) {
                    (true, false) => CmpOp::Greater,
                    (true, true) => CmpOp::GreaterEq,
                    (false, false) => CmpOp::Smaller,
                    (false, true) => CmpOp::SmallerEq,
                }
            }
            _ => CmpOp::Is,
        };
        let rhs = self.parse_when_operand()?;
        Ok(Cond::Cmp(lhs, op, rhs))
    }

    /// `System::NOM` ou littéral chaîne / entier.
    fn parse_when_operand(&mut self) -> ParseResult<Operand> {
        let span = self.span();
        match self.peek_kind().clone() {
            TokenKind::LitString(s) => { self.advance(); Ok(Operand::Value(s)) }
            TokenKind::LitInt(n) => { self.advance(); Ok(Operand::Value(n.to_string())) }
            TokenKind::Ident(class) if class == "System" => {
                self.advance();
                self.eat(&TokenKind::ColonColon)?;
                let (name, name_span) = self.eat_ident()?;
                if !build::DOMAINS.iter().any(|(n, _)| *n == name) {
                    return Err(ParseError::new(format!("clause 'when' : constante de build inconnue 'System::{}' (attendu System::OS, System::ARCH ou System::BUILD)", name), name_span));
                }
                Ok(Operand::Const(name))
            }
            _ => Err(ParseError::new("clause 'when' : constante de build (System::OS, System::ARCH, System::BUILD) ou littéral attendu", span)),
        }
    }

    /// Membres d'un corps de classe, generic ou module ; l'initialiseur
    /// d'une `property` écartée l'est avec elle.
    pub(super) fn parse_members(&mut self) -> ParseResult<Vec<ClassMember>> {
        // Reste d'un corps abandonné sur erreur (mode tolérant).
        self.class_wirings.clear();
        let mut entries = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            // `wiring <chemin>` : réservé aux classes de test, repris par
            // `parse_class` (refusé ailleurs).
            if self.check_exact(&TokenKind::Wiring) {
                let wiring = self.parse_wiring_decl()?;
                self.class_wirings.push(wiring);
                continue;
            }
            let start = self.pos;
            let before = self.property_initializers.len();
            match self.parse_member_variant() {
                Ok((variant, member)) => {
                    let inits: Vec<_> = self.property_initializers.drain(before..).collect();
                    entries.push((variant, (member, inits)));
                }
                Err(e) => {
                    self.property_initializers.truncate(before);
                    self.recover_member(e, start)?;
                }
            }
        }
        let (variants, items): (Vec<Variant>, Vec<_>) = entries.into_iter().unzip();
        let keep = self.select_or_note(&variants)?;
        let mut members = Vec::new();
        for ((member, inits), keep) in items.into_iter().zip(keep) {
            if keep {
                members.push(member);
                self.property_initializers.extend(inits);
            }
        }
        Ok(members)
    }

    fn parse_member_variant(&mut self) -> ParseResult<(Variant, ClassMember)> {
        let clause = self.parse_when_clause()?;
        let span = self.span();
        let member = self.parse_class_member()?;
        let key = match &member {
            ClassMember::Field { name, .. } => format!("prop:{}", name),
            ClassMember::Const { name, .. } => format!("const:{}", name),
            ClassMember::Method { decl, .. } => format!("method:{}", decl.name),
            ClassMember::Constructor { .. } => "init".to_string(),
        };
        Ok((Variant { key, clause, span, end_line: self.end_line() }, member))
    }

    /// Instructions jusqu'à `}` : une suite d'instructions conditionnées
    /// consécutives forme un groupe de variantes ; une instruction sans
    /// clause ou un `when default` le termine.
    pub(super) fn parse_stmts(&mut self) -> ParseResult<Vec<Stmt>> {
        let mut entries = Vec::new();
        let mut group = 0;
        let mut in_group = false;
        while !self.check_exact(&TokenKind::RBrace) && !self.check_exact(&TokenKind::Eof) {
            let start = self.pos;
            let parsed = self.parse_when_clause().and_then(|clause| {
                let span = self.span();
                self.parse_stmt().map(|stmt| (clause, span, stmt))
            });
            match parsed {
                Ok((clause, span, stmt)) => {
                    let conditioned = !matches!(clause, Clause::None);
                    if !(conditioned && in_group) {
                        group += 1;
                    }
                    // `when default` (le « sinon ») clôt aussi le groupe.
                    in_group = matches!(clause, Clause::Cond(..));
                    entries.push((Variant { key: format!("stmt:{}", group), clause, span, end_line: self.end_line() }, stmt));
                }
                Err(e) => self.recover_stmt(e, start)?,
            }
        }
        let (variants, stmts): (Vec<Variant>, Vec<Stmt>) = entries.into_iter().unzip();
        let keep = self.select_or_note(&variants)?;
        Ok(stmts.into_iter().zip(keep).filter(|(_, k)| *k).map(|(s, _)| s).collect())
    }

    pub(super) fn end_line(&self) -> usize {
        self.previous_line().unwrap_or(0)
    }

    /// Variantes retenues parmi `variants` (même ordre) ; groupes de même
    /// clé sans aucune clause `when` gardés tels quels.
    pub(super) fn select_variants(&self, variants: &[Variant]) -> ParseResult<Vec<bool>> {
        let config = build::current();
        let mut keep = vec![true; variants.len()];
        // Groupes par clé, dans l'ordre de première occurrence — une passe
        // (un balayage par clé rendait un bloc de n instructions quadratique).
        let mut group_of: HashMap<&str, usize> = HashMap::new();
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for (j, v) in variants.iter().enumerate() {
            let index = *group_of.entry(v.key.as_str()).or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            groups[index].push(j);
        }
        for group in groups {
            let i = group[0];
            if group.iter().all(|&j| matches!(variants[j].clause, Clause::None)) {
                continue;
            }
            let fallbacks: Vec<usize> = group.iter().copied().filter(|&j| !matches!(variants[j].clause, Clause::Cond(..))).collect();
            if let Some(&second) = fallbacks.get(1) {
                let span = match &variants[second].clause {
                    Clause::Default(s) => s.clone(),
                    _ => variants[second].span.clone(),
                };
                return Err(ParseError::new("plusieurs variantes « sinon » (sans clause 'when' ou 'when default') pour la même déclaration", span));
            }
            if group.len() == 1 {
                if let Clause::Default(span) = &variants[i].clause {
                    build::note_warning(span, "'when default' sans autre variante : sans effet".into());
                }
            }
            let mut winner = None;
            for &j in &group {
                if let Clause::Cond(cond, _) = &variants[j].clause {
                    if winner.is_none() && eval(cond, &config) {
                        winner = Some(j);
                    }
                }
            }
            let winner = winner.or(fallbacks.first().copied());
            for &j in &group {
                let retained = winner == Some(j);
                keep[j] = retained || !config.select;
                let start = match &variants[j].clause {
                    Clause::Cond(_, span) | Clause::Default(span) => span,
                    Clause::None => &variants[j].span,
                };
                build::note_variant(start, variants[j].end_line, retained);
            }
        }
        Ok(keep)
    }
}

fn eval(cond: &Cond, config: &build::BuildConfig) -> bool {
    match cond {
        Cond::And(a, b) => eval(a, config) & eval(b, config),
        Cond::Or(a, b) => eval(a, config) | eval(b, config),
        Cond::Cmp(lhs, op, rhs) => {
            note(lhs, rhs);
            note(rhs, lhs);
            let (l, r) = (value(lhs, config), value(rhs, config));
            let ord = match (l.parse::<i64>(), r.parse::<i64>()) {
                (Ok(a), Ok(b)) => a.cmp(&b),
                _ => l.cmp(&r),
            };
            match op {
                CmpOp::Is => l == r,
                CmpOp::IsNot => l != r,
                CmpOp::Greater => ord.is_gt(),
                CmpOp::Smaller => ord.is_lt(),
                CmpOp::GreaterEq => ord.is_ge(),
                CmpOp::SmallerEq => ord.is_le(),
            }
        }
    }
}

/// Journalise la constante lue et la valeur à laquelle elle est comparée.
fn note(a: &Operand, b: &Operand) {
    if let Operand::Const(name) = a {
        let value = match b {
            Operand::Value(v) => Some(v.as_str()),
            Operand::Const(_) => None,
        };
        build::note_used(name, value);
    }
}

fn value(operand: &Operand, config: &build::BuildConfig) -> String {
    match operand {
        Operand::Const(name) => config.get(name).unwrap_or("").to_string(),
        Operand::Value(v) => v.clone(),
    }
}
