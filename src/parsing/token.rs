use std::fmt;

// ─────────────────────────────────────────────────────────────────────────────
// Span  –  position (line/col, 1-based) dans le fichier source
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub file: Option<String>,
    pub runtime_ctx: Option<String>,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col, file: None, runtime_ctx: None }
    }
    
    #[allow(dead_code)]
    pub fn with_file(line: usize, col: usize, file: String) -> Self {
        Self { line, col, file: Some(file), runtime_ctx: None }
    }

    /// Combine deux spans pour créer un span qui les couvre tous les deux.
    /// Pour simplifier, on garde le début du premier et la fin du second.
    pub fn union(&self, other: &Span) -> Span {
        // Le span résultant couvre du début du premier à la fin du second
        // Pour simplifier, on retourne le second span (qui vient après)
        // car il représente la fin de l'expression combinée
        other.clone()
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
    From,
    Namespace,
    As,
    Var,
    Scoped,
    Consumed,
    Property,
    Const,
    Function,
    Method,
    Class,
    Generic,
    Module,
    Enum,
    Interface,
    Extends,
    Modules,
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
    Result,     // result (bloc runtime : fixe ERROR sans quitter, remplace `return`)
    Use,
    Break,
    Continue,
    Try,
    On,
    Is,
    Raise,
    SelfKw, // self
    ParentKw, // parent
    Async,
    Resolve,
    Variadic,

    // ── Blocs runtime ─────────────────────────────────────────────────────────
    Runtime,
    Main,
    Error,
    Success,
    Exit,

    // ── Types primitifs ───────────────────────────────────────────────────────
    TInt,
    TFloat,
    TString,
    TBool,
    TMixed,
    TArray,
    TMap,
    TVoid,

    // ── Littéraux ─────────────────────────────────────────────────────────────
    LitInt(i64),
    LitFloat(f64),
    LitString(String),
    LitTrue,
    LitFalse,
    LitNull,
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

    // ── Opérateurs de comparaison symboliques (SUPPRIMÉS du langage) ───────────
    // Depuis Ocara v0.2.0, toute comparaison s'écrit en toutes lettres
    // (`equal`, `not equal`, `smaller`, `greater`, `smaller or equal`,
    // `greater or equal`) — comparaisons toujours typées à la compilation.
    // Ces variantes ne sont plus produites par `parse_comparison` que pour
    // détecter leur usage et renvoyer un message de migration explicite ;
    // `Lt`/`Gt` restent utilisés ailleurs pour les génériques (`List<T>`).
    EqEq,   // == (supprimé — utiliser 'equal')
    BangEq, // != (supprimé — utiliser 'not equal')
    EqEqEq, // === (supprimé — utiliser 'equal')
    BangEqEq, // !== (supprimé — utiliser 'not equal')
    Lt,     // < : délimiteur de génériques (List<T>) ; supprimé en comparaison
    Gt,     // > : délimiteur de génériques (List<T>) ; supprimé en comparaison
    LtEq,   // <= (supprimé — utiliser 'smaller or equal')
    GtEq,   // >= (supprimé — utiliser 'greater or equal')
    LtEqEq, // <== (supprimé — utiliser 'smaller or equal')
    GtEqEq, // >== (supprimé — utiliser 'greater or equal')

    // ── Opérateurs logiques et de comparaison littéraux ────────────────────────
    KwAnd, // and
    KwOr,  // or
    KwNot,      // not
    KwEqual,    // equal        — égalité typée
    KwSmaller,  // smaller      — inférieur, typé
    KwGreater,  // greater      — supérieur, typé
    KwNameless, // nameless

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
    Pipe,        // |    (union de types)

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
