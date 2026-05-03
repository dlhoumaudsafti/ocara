/// Statements de l'AST Ocara

use super::expressions::Expr;
use super::literals::Literal;
use super::types::Type;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Statements
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `var x:T = expr`
    Var {
        name:    String,
        ty:      Type,
        value:   Expr,
        mutable: bool,     // true = var, false = let
        span:    Span,
    },

    /// `const X:T = expr`
    Const {
        name:  String,
        ty:    Type,
        value: Expr,
        span:  Span,
    },

    /// Appel d'expression utilisé comme statement
    Expr(Expr),

    /// `if expr { } elseif expr { } else { }`
    If {
        condition:  Expr,
        then_block: Block,
        elseif:     Vec<(Expr, Block)>,
        else_block: Option<Block>,
        span:       Span,
    },

    /// `switch expr { lit { } default { } }`
    Switch {
        subject:  Expr,
        cases:    Vec<SwitchCase>,
        default:  Option<Block>,
        span:     Span,
    },

    /// `while expr { }`
    While {
        condition: Expr,
        body:      Block,
        span:      Span,
    },

    /// `for i in expr { }`
    ForIn {
        var:  String,
        iter: Expr,
        body: Block,
        span: Span,
    },

    /// `for k => v in expr { }`
    ForMap {
        key:   String,
        value: String,
        iter:  Expr,
        body:  Block,
        span:  Span,
    },

    /// `return expr`
    Return {
        value: Option<Expr>,
        span:  Span,
    },

    /// `break` — sortie immédiate de la boucle courante
    Break { span: Span },

    /// `continue` — passe à l'itération suivante de la boucle courante
    Continue { span: Span },

    /// `try { } on e [is Foo] { }`
    Try {
        body:     Block,
        handlers: Vec<OnClause>,
        span:     Span,
    },

    /// `raise expr`
    Raise {
        value: Expr,
        span:  Span,
    },

    /// Affectation : `target = value`
    /// target peut être Ident, Field ou Index
    Assign {
        target: Expr,
        value:  Expr,
        span:   Span,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Clause `on` d'un bloc try
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct OnClause {
    /// Nom de la variable d'erreur : `on e { }` → binding = "e"
    pub binding:      String,
    /// Filtre de classe optionnel : `on e is IOException { }` → Some("IOException")
    pub class_filter: Option<String>,
    pub body:         Block,
    pub span:         Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Cas de switch
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub pattern: Literal,
    pub body:    Block,
    pub span:    Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Block
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span:  Span,
}
