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
        /// Arguments de type explicites d'un appel statique sur un `generic`
        /// (`Box<int>::of(4)`), ou inférés par la sema ; vide sinon.
        type_args: Vec<Type>,
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

    /// Expression conditionnelle : `cond then a else b`, ou forme courte
    /// `cond then a empty|zero|null` — voir `docs/EBNF.md` §11.3.
    Cond {
        cond:      Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: CondElse,
        span:      Span,
    },

    /// `self`
    SelfExpr(Span),

    /// `parent` — référence à l'instance parent dans héritage
    ParentExpr(Span),

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

    /// Incrémentation/décrémentation, préfixe ou suffixe : `i++`, `++i`,
    /// `i--`, `--i` — voir docs/roadmap.d/langage-increment-decrement.md.
    /// `target` doit être un `Ident`/`Field`/`Index` (validé en sema, comme
    /// pour `Stmt::Assign` — le parser reste générique).
    IncDec {
        op:     IncDecOp,
        target: Box<Expr>,
        span:   Span,
    },

    /// Argument nommé à l'appel : `name: expr` — n'apparaît QUE comme élément
    /// direct de `args` d'un `Call`/`StaticCall`/`New`. Résolu en positionnel
    /// par la sema (`sema::named_args`), puis réécrit dans l'AST avant le
    /// lowering (`core::named_args`) : le lowering n'en voit jamais.
    NamedArg {
        name:  String,
        value: Box<Expr>,
        span:  Span,
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
            Expr::Cond { span, .. } => span,
            Expr::SelfExpr(span) => span,
            Expr::ParentExpr(span) => span,
            Expr::Nameless { span, .. } => span,
            Expr::Resolve { span, .. } => span,
            Expr::IsCheck { span, .. } => span,
            Expr::IncDec { span, .. } => span,
            Expr::NamedArg { span, .. } => span,
        }
    }
}

/// Branche `else` d'un `Expr::Cond`.
#[derive(Debug, Clone, PartialEq)]
pub enum CondElse {
    /// `else <valeur>`.
    Value(Box<Expr>),
    /// Forme courte (`empty`, `zero`, `null`, position du mot) : valeur par
    /// défaut du type de la branche `then`, remplacée par un littéral après
    /// la sema (`AstRewrites::cond_defaults`) — le lowering n'en voit jamais.
    Default(CondDefault, Span),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondDefault {
    Empty,
    Zero,
    Null,
}

impl CondDefault {
    pub fn keyword(self) -> &'static str {
        match self {
            CondDefault::Empty => "empty",
            CondDefault::Zero  => "zero",
            CondDefault::Null  => "null",
        }
    }
}

impl CondElse {
    pub fn value(&self) -> Option<&Expr> {
        match self {
            CondElse::Value(e) => Some(e),
            CondElse::Default(..) => None,
        }
    }

    pub fn value_mut(&mut self) -> Option<&mut Expr> {
        match self {
            CondElse::Value(e) => Some(e),
            CondElse::Default(..) => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Opérateurs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    /// Produit uniquement par `x -= e` : soustraction, ou suppression de
    /// toutes les occurrences de `e` si `x` est une `string`. Résolu par la
    /// sema puis réécrit (`Sub` ou `String::replace`) avant le lowering.
    Remove,
    // Comparaisons : toujours typées à la compilation (voir sema::typecheck).
    // Un seul mot-clé par opérateur — `equal`/`smaller`/`greater`/
    // `smaller or equal`/`greater or equal`/`not equal` — aucun symbole.
    Equal, NotEqual, Smaller, Greater, SmallerOrEqual, GreaterOrEqual,
    And, Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
}

/// Forme de `Expr::IncDec` — préfixe (`++i`/`--i`, la nouvelle valeur est le
/// résultat) ou suffixe (`i++`/`i--`, l'ancienne valeur est le résultat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncDecOp {
    PreInc,
    PreDec,
    PostInc,
    PostDec,
}

impl IncDecOp {
    pub fn is_increment(self) -> bool {
        matches!(self, IncDecOp::PreInc | IncDecOp::PostInc)
    }

    pub fn is_prefix(self) -> bool {
        matches!(self, IncDecOp::PreInc | IncDecOp::PreDec)
    }
}
