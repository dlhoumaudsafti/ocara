use std::fmt;

// ─────────────────────────────────────────────────────────────────────────────
// Span  –  position (line/col, 1-based) dans le fichier source
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TemplatePart — fragment d'une chaîne template
// ─────────────────────────────────────────────────────────────────────────────

/// Fragment d'une chaîne template `` `...${expr}...` ``
#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    /// Texte brut entre les interpolations
    Literal(String),
    /// Source brut de l'expression entre `${` et `}`
    ExprSrc(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// TokenKind  –  tous les tokens Ocara v1.0
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── Mots-clés ────────────────────────────────────────────────────────────
    Import,
    As,
    Var,
    Scoped,
    Property,
    Const,
    Function,
    Method,
    Class,
    Interface,
    Extends,
    Implements,
    Init,
    Public,
    Private,
    Protected,
    Static,
    If,
    Elseif,
    Else,
    Switch,
    Default,
    Match,
    While,
    For,
    In,
    Return,
    Use,
    Break,
    Continue,
    Try,
    On,
    Is,
    Raise,
    SelfKw, // self

    // ── Types primitifs ───────────────────────────────────────────────────────
    TInt,
    TFloat,
    TString,
    TBool,
    TMixed,
    TMap,
    TVoid,

    // ── Littéraux ─────────────────────────────────────────────────────────────
    LitInt(i64),
    LitFloat(f64),
    LitString(String),
    LitTrue,
    LitFalse,
    /// Chaîne template : `` `Bonjour ${name} !` ``
    /// Chaque partie est soit un texte brut, soit le source d'une expression.
    LitTemplate(Vec<TemplatePart>),

    // ── Identifiant ───────────────────────────────────────────────────────────
    Ident(String),

    // ── Opérateurs arithmétiques ──────────────────────────────────────────────
    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /
    Percent, // %

    // ── Opérateurs de comparaison ─────────────────────────────────────────────
    EqEq,   // ==
    BangEq, // !=
    Lt,     // <
    Gt,     // >
    LtEq,   // <=
    GtEq,   // >=

    // ── Opérateurs logiques ───────────────────────────────────────────────────
    And,  // &&
    Or,   // ||
    Bang, // !

    // ── Affectation ───────────────────────────────────────────────────────────
    Eq, // =

    // ── Ponctuation ───────────────────────────────────────────────────────────
    LParen,      // (
    RParen,      // )
    LBrace,      // {
    RBrace,      // }
    LBracket,    // [
    RBracket,    // ]
    Comma,       // ,
    Colon,       // :
    Dot,         // .
    DotDot,      // ..   (range)
    Arrow,       // =>   (match arm / map for)
    ColonColon,  // ::   (accès statique)

    // ── Fin de fichier ────────────────────────────────────────────────────────
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::LitInt(n)      => write!(f, "LitInt({})", n),
            TokenKind::LitFloat(n)    => write!(f, "LitFloat({})", n),
            TokenKind::LitString(s)   => write!(f, "LitString({:?})", s),
            TokenKind::LitTemplate(_) => write!(f, "LitTemplate"),
            TokenKind::Ident(s)       => write!(f, "Ident({})", s),
            other                     => write!(f, "{:?}", other),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Token  –  kind + lexème brut + position
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind:   TokenKind,
    pub lexeme: String,
    pub span:   Span,
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: impl Into<String>, span: Span) -> Self {
        Self { kind, lexeme: lexeme.into(), span }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:>4}:{:<3}]  {:<28}  {:?}",
            self.span.line, self.span.col,
            self.kind.to_string(),
            self.lexeme
        )
    }
}
