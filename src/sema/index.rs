//! Index des références résolues par la sema (serveur de langage) : pour
//! chaque nom utilisé, ce qu'il désigne et son type. Collecté seulement si
//! `TypeChecker::index` est activé.

use crate::parsing::ast::Type;
use crate::parsing::token::Span;

#[derive(Debug, Clone)]
pub enum Target {
    /// Variable locale ou paramètre, avec la position de sa déclaration.
    Local { decl: Span },
    Const(String),
    Class(String),
    Function(String),
    /// Méthode appelée sur une valeur de type `class` (déclarée dans `class`
    /// ou un de ses parents).
    Method { class: String, name: String },
    Field { class: String, name: String },
    ClassConst { class: String, name: String },
    /// Argument nommé : paramètre `name` de `callee` (`f` ou `Classe::m`).
    Param { callee: String, name: String },
}

#[derive(Debug, Clone)]
pub struct Reference {
    /// Position rapportée par le parseur, au nom ou juste avant lui (point
    /// d'un accès `a.b`, classe d'un `A::b`) : le nom suit sur la même ligne.
    pub span: Span,
    pub name: String,
    pub target: Target,
    pub ty: Type,
}

impl<'a> crate::sema::typecheck::TypeChecker<'a> {
    pub(crate) fn record(&mut self, span: &Span, name: &str, target: Target, ty: &Type) {
        if let Some(index) = self.index.as_mut() {
            index.push(Reference { span: span.clone(), name: name.to_string(), target, ty: ty.clone() });
        }
    }
}
