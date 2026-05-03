/// Déclarations de types génériques

use super::types::{Type, TypeParam};
use super::classes::ClassMember;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Déclaration générique (ex: generic List<T> { ... })
// ─────────────────────────────────────────────────────────────────────────────

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
