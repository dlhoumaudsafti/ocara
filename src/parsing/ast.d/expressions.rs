/// Expressions de l'AST Ocara

use super::literals::{Literal, TemplatePartExpr};
use super::types::Type;
use super::patterns::{MatchArm};
use super::statements::Block;
use super::params::Param;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Expressions
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Littéral : `42`, `3.14`, `"hello"`, `true`
    Literal(Literal, Span),

    /// Identifiant simple : `x`
    Ident(String, Span),

    /// Accès de membre : `user.age`
    Field {
        object: Box<Expr>,
        field:  String,
        span:   Span,
    },

    /// Appel de méthode / fonction simple : `foo(a, b)`
    Call {
        callee: Box<Expr>,
        args:   Vec<Expr>,
        span:   Span,
    },

    /// Accès statique puis appel : `Math::abs(x)`
    StaticCall {
        class:  String,
        method: String,
        args:   Vec<Expr>,
        span:   Span,
    },

    /// Lecture d'une constante de classe : `Test::NAME`
    StaticConst {
        class: String,
        name:  String,
        span:  Span,
    },

    /// Instanciation : `use Foo(a, b)` ou `use Cache<int, User>()`
    New {
        class:     String,
        type_args: Vec<Type>,
        args:      Vec<Expr>,
        span:      Span,
    },

    /// Opération binaire
    Binary {
        op:    BinOp,
        left:  Box<Expr>,
        right: Box<Expr>,
        span:  Span,
    },

    /// Négation logique : `!x`
    Unary {
        op:      UnaryOp,
        operand: Box<Expr>,
        span:    Span,
    },

    /// Tableau littéral : `[1, 2, 3]`
    Array {
        elements: Vec<Expr>,
        span:     Span,
    },

    /// Tableau associatif littéral : `{"name": "Lucas"}`
    Map {
        entries: Vec<(Expr, Expr)>,
        span:    Span,
    },

    /// Chaîne template : `` `Bonjour ${name} !` ``
    Template {
        parts: Vec<TemplatePartExpr>,
        span:  Span,
    },

    /// Accès par index : `arr[0]` / `map["key"]`
    Index {
        object: Box<Expr>,
        index:  Box<Expr>,
        span:   Span,
    },

    /// Plage : `0..5`
    Range {
        start: Box<Expr>,
        end:   Box<Expr>,
        span:  Span,
    },

    /// Expression `match`
    Match {
        subject: Box<Expr>,
        arms:    Vec<MatchArm>,
        span:    Span,
    },

    /// `self`
    SelfExpr(Span),

    /// Fonction anonyme (closure) : `nameless(params): ret { body }`
    Nameless {
        params: Vec<Param>,
        ret_ty: Option<Type>,
        body:   Block,
        span:   Span,
    },

    /// `resolve expr` — attend la fin d'une tâche async et retourne son résultat
    Resolve {
        expr: Box<Expr>,
        span: Span,
    },

    /// Test de type runtime : `val is int`, `obj is null`
    IsCheck {
        expr: Box<Expr>,
        ty:   Type,
        span: Span,
    },
}

impl Expr {
    /// Retourne le span de l'expression
    pub fn span(&self) -> &Span {
        match self {
            Expr::Literal(_, span) => span,
            Expr::Ident(_, span) => span,
            Expr::Field { span, .. } => span,
            Expr::Call { span, .. } => span,
            Expr::StaticCall { span, .. } => span,
            Expr::StaticConst { span, .. } => span,
            Expr::New { span, .. } => span,
            Expr::Binary { span, .. } => span,
            Expr::Unary { span, .. } => span,
            Expr::Array { span, .. } => span,
            Expr::Map { span, .. } => span,
            Expr::Template { span, .. } => span,
            Expr::Index { span, .. } => span,
            Expr::Range { span, .. } => span,
            Expr::Match { span, .. } => span,
            Expr::SelfExpr(span) => span,
            Expr::Nameless { span, .. } => span,
            Expr::Resolve { span, .. } => span,
            Expr::IsCheck { span, .. } => span,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Opérateurs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    EqEq, NotEq, Lt, LtEq, Gt, GtEq,
    EqEqEq, NotEqEq, LtEqEq, GtEqEq, // Opérateurs stricts avec vérification de type
    And, Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
}
