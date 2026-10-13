/// Visibilité des champs (E54) — un champ `private` n'est accessible (lecture
/// comme affectation) que depuis la classe qui le déclare ; un champ
/// `protected`, depuis cette classe et ses descendantes (`extends`, classes
/// comme structs). Voir docs/roadmap.d/langage-field-visibility-unchecked.md.
///
/// Un champ apporté par un module (mixin) est composé dans chaque classe qui
/// l'utilise (voir `SymbolTable::register_class`) : la classe déclarante est
/// alors la classe utilisatrice elle-même.
///
/// Même règle pour les méthodes (instance et statiques) et les constantes de
/// classe (`SemaError::MemberNotAccessible`, E54).

use std::collections::HashMap;
use crate::parsing::ast::{ClassMember, Program, Visibility};
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

    /// `class.method(...)`/`Class::method(...)` : visibilité de la méthode,
    /// cherchée dans `class` puis ses ancêtres (classe déclarante).
    pub(crate) fn check_method_visibility(&mut self, class: &str, method: &str, span: &Span) {
        let mut owner = class.to_string();
        loop {
            if let Some(vis) = self.method_vis.get(&owner).and_then(|m| m.get(method)).cloned() {
                self.check_member_visibility("method", &owner, &vis, method, span);
                return;
            }
            match self.symbols.lookup_parent_class(&owner) {
                Some(parent) => owner = parent,
                None => return,
            }
        }
    }

    /// Méthode ou constante de classe déclarée dans `owner`.
    pub(crate) fn check_member_visibility(&mut self, kind: &str, owner: &str, vis: &Visibility, name: &str, span: &Span) {
        let accessible = match vis {
            Visibility::Public => true,
            Visibility::Private => self.current_class.as_deref() == Some(owner),
            Visibility::Protected => self.current_class.as_deref()
                .is_some_and(|current| self.symbols.class_matches(current, owner)),
        };
        if !accessible {
            self.errors.push(SemaError::MemberNotAccessible {
                kind:      kind.to_string(),
                class:     owner.to_string(),
                name:      name.to_string(),
                protected: matches!(vis, Visibility::Protected),
                span:      span.clone(),
            });
        }
    }
}

/// `classe → { méthode → visibilité }` des classes du programme (méthodes
/// des modules composées dans chaque classe qui les utilise).
pub fn collect_method_visibility(program: &Program) -> HashMap<String, HashMap<String, Visibility>> {
    let mut table: HashMap<String, HashMap<String, Visibility>> = HashMap::new();
    let mut insert = |owner: &str, members: &[ClassMember]| {
        for member in members {
            if let ClassMember::Method { vis, decl, .. } = member {
                table.entry(owner.to_string()).or_default().entry(decl.name.clone()).or_insert_with(|| vis.clone());
            }
        }
    };
    for c in &program.classes {
        insert(&c.name, &c.members);
        for module in program.modules.iter().filter(|m| c.modules.contains(&m.name)) {
            insert(&c.name, &module.members);
        }
    }
    table
}
