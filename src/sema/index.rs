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
    /// Marqueur de complétion (`COMPLETION_MARKER`) : membres du type de la
    /// référence (`a.`), membres statiques (`A::`), ou noms visibles.
    Completion(Completion),
}

#[derive(Debug, Clone)]
pub enum Completion {
    Member,
    Static,
    /// Variables locales et paramètres visibles, du plus proche au plus loin.
    Scope(Vec<(String, Type)>),
}

/// Identifiant inséré par le serveur de langage à la place du nom en cours
/// de frappe, pour que le texte se parse et que la sema type son contexte.
pub const COMPLETION_MARKER: &str = "__ocara_cursor";

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
