use crate::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Types Ocara v1.0
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Mixed,
    Void,
    Null,
    /// Type nommé (classe, interface, alias d'import)
    Named(String),
    /// Type qualifié : `repository.User`
    Qualified(Vec<String>),
    /// `Type[]`
    Array(Box<Type>),
    /// `map<K, V>`
    Map(Box<Type>, Box<Type>),
    /// Type générique avec arguments : `List<int>`, `Cache<string, User>`
    Generic {
        name: String,
        args: Vec<Type>,
    },
    /// `T | U | ...` — type union
    Union(Vec<Type>),
    /// Référence à une fonction ou méthode statique (premier ordre) :
    /// Syntaxe : `Function<ReturnType(ParamType1, ParamType2, ...)>`
    Function {
        ret_ty: Box<Type>,
        param_tys: Vec<Type>,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Visibilité
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
}

// ─────────────────────────────────────────────────────────────────────────────
// Littéraux
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

// ─────────────────────────────────────────────────────────────────────────────
// TemplatePartExpr — fragment AST d'une chaîne template
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePartExpr {
    Literal(String),
    Expr(Box<Expr>),
}

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

// ─────────────────────────────────────────────────────────────────────────────
// Pattern de match
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    /// Littéral : `42`, `"hello"`, `true`, `null`
    Literal(Literal),
    /// Test de type : `is int`, `is string`, `is null`, `is ClassName`
    IsType(Type),
}

// ─────────────────────────────────────────────────────────────────────────────
// Bras de match
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// `None` → bras `default`
    pub pattern: Option<MatchPattern>,
    pub body:    Expr,
    pub span:    Span,
}

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

// ─────────────────────────────────────────────────────────────────────────────
// Paramètre de fonction / constructeur
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name:          String,
    pub ty:            Type,
    pub default_value: Option<Expr>,
    pub is_variadic:   bool,
    pub span:          Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Déclarations de haut niveau
// ─────────────────────────────────────────────────────────────────────────────

/// Déclaration de fonction de niveau module
#[derive(Debug, Clone, PartialEq)]
pub struct FuncDecl {
    pub name:     String,
    pub params:   Vec<Param>,
    pub ret_ty:   Type,
    pub body:     Block,
    pub is_async: bool,
    pub span:     Span,
}

/// Membre d'une classe
#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    Field {
        vis:     Visibility,
        mutable: bool,
        name:    String,
        ty:      Type,
        span:    Span,
    },
    Const {
        vis:   Visibility,
        name:  String,
        ty:    Type,
        value: Expr,
        span:  Span,
    },
    Method {
        vis:       Visibility,
        is_static: bool,
        decl:      FuncDecl,
        span:      Span,
    },
    Constructor {
        params: Vec<Param>,
        body:   Block,
        span:   Span,
    },
}

/// Déclaration de classe
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub name:       String,
    pub extends:    Option<String>,
    pub modules:    Vec<String>,
    pub implements: Vec<String>,
    pub members:    Vec<ClassMember>,
    pub span:       Span,
}

/// Paramètre de type générique (ex: T, K, V = string)
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name:    String,
    /// Valeur par défaut optionnelle
    pub default: Option<Type>,
    pub span:    Span,
}

/// Déclaration générique (ex: generic List<T> { ... })
#[derive(Debug, Clone, PartialEq)]
pub struct GenericDecl {
    pub name:        String,
    pub type_params: Vec<TypeParam>,
    pub extends:     Option<String>,
    /// Arguments de type pour extends (ex: extends Base<T>)
    pub extends_args: Vec<Type>,
    pub modules:     Vec<String>,
    pub implements:  Vec<String>,
    pub members:     Vec<ClassMember>,
    pub span:        Span,
}

/// Déclaration de module (mixin)
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    pub name:    String,
    pub members: Vec<ClassMember>,
    pub span:    Span,
}

/// Méthode d'interface (signature seule)
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceMethod {
    pub name:   String,
    pub params: Vec<Param>,
    pub ret_ty: Type,
    pub span:   Span,
}

/// Déclaration d'interface
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    pub name:    String,
    pub methods: Vec<InterfaceMethod>,
    pub span:    Span,
}

/// Variante d'un enum
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name:  String,
    /// Valeur explicite, ou None → valeur auto (index)
    pub value: Option<i64>,
    pub span:  Span,
}

/// Déclaration d'un enum
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name:     String,
    pub variants: Vec<EnumVariant>,
    pub span:     Span,
}

/// Déclaration de constante globale
#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name:  String,
    pub ty:    Type,
    pub value: Expr,
    pub span:  Span,
}

/// Import
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    /// Chemin qualifié ou noms à importer
    /// - Format ancien: `["ocara", "IO"]` pour `import ocara.IO`
    /// - Format nouveau: `["Circle"]` pour `import Circle from "file"`
    /// - Format wildcard: `["*"]` pour `import * from "file"`
    pub path:  Vec<String>,
    /// Chemin de fichier optionnel pour `from "path"`
    /// Ex: Some("11_interfaces.oc") pour `import Circle from "11_interfaces.oc"`
    pub file_path: Option<String>,
    /// Alias optionnel : `as UserData`
    pub alias: Option<String>,
    pub span:  Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Programme (racine de l'AST)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub namespace:  Option<String>, // None ou "." = racine, "classes" = namespace classes, etc.
    pub imports:    Vec<ImportDecl>,
    pub consts:     Vec<ConstDecl>,
    pub modules:    Vec<ModuleDecl>,
    pub enums:      Vec<EnumDecl>,
    pub classes:    Vec<ClassDecl>,
    pub generics:   Vec<GenericDecl>,
    pub interfaces: Vec<InterfaceDecl>,
    pub functions:  Vec<FuncDecl>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            namespace:  None,
            imports:    Vec::new(),
            consts:     Vec::new(),
            modules:    Vec::new(),
            enums:      Vec::new(),
            classes:    Vec::new(),
            generics:   Vec::new(),
            interfaces: Vec::new(),
            functions:  Vec::new(),
        }
    }
}
