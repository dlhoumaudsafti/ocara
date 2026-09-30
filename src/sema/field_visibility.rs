/// Visibilité des champs (E54) — un champ `private` n'est accessible (lecture
/// comme affectation) que depuis la classe qui le déclare ; un champ
/// `protected`, depuis cette classe et ses descendantes (`extends`, classes
/// comme structs). Voir docs/roadmap.d/langage-field-visibility-unchecked.md.
///
/// Un champ apporté par un module (mixin) est composé dans chaque classe qui
/// l'utilise (voir `SymbolTable::register_class`) : la classe déclarante est
/// alors la classe utilisatrice elle-même.

use crate::parsing::ast::Visibility;
use crate::parsing::token::Span;
use crate::sema::error::SemaError;
use crate::sema::typecheck::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_field_visibility(&mut self, owner: &str, vis: &Visibility, field: &str, span: &Span) {
        let accessible = match vis {
            Visibility::Public => true,
            Visibility::Private => self.current_class.as_deref() == Some(owner),
            Visibility::Protected => self.current_class.as_deref()
                .is_some_and(|current| self.symbols.class_matches(current, owner)),
        };
        if !accessible {
            self.errors.push(SemaError::FieldNotAccessible {
                class:     owner.to_string(),
                field:     field.to_string(),
                protected: matches!(vis, Visibility::Protected),
                span:      span.clone(),
            });
        }
    }
}
