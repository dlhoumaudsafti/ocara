/// Types et visibilité pour l'AST Ocara

// ─────────────────────────────────────────────────────────────────────────────
// Types Ocara v0.1.0
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
// Paramètre de type générique
// ─────────────────────────────────────────────────────────────────────────────

use crate::parsing::token::Span;

/// Paramètre de type générique (ex: T, K, V = string)
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name:    String,
    /// Valeur par défaut optionnelle
    pub default: Option<Type>,
    pub span:    Span,
}
