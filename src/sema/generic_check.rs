/// Vérification sémantique du corps des `generic` et des `module` (mixins),
/// jusqu'ici jamais parcourus par l'analyse sémantique — voir
/// docs/roadmap.d/sema-generic-bodies-unchecked.md.
///
/// Chaque `generic` est vérifié UNE fois, qu'il soit instancié ou non, sous
/// sa forme « effacée » (`core::monomorph::erased_generic_class`) : ses
/// paramètres de type y valent `mixed` (permissif). Tout ce qui ne dépend
/// pas de `T` est donc contrôlé comme dans une classe ordinaire (symboles,
/// arité, types concrets, visibilité, arguments nommés...) ; une opération
/// sur une valeur de type `T` est acceptée. Les diagnostics propres à
/// `mixed` sont suspendus pendant cette vérification (un `T` n'est pas un
/// `mixed` écrit par le développeur).

use crate::parsing::ast::{ClassDecl, GenericDecl, ModuleDecl};
use crate::sema::typecheck::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_generic(&mut self, generic: &GenericDecl) {
        let erased = crate::core::monomorph::erased_generic_class(generic);
        let saved = self.current_generic.replace((generic.name.clone(), generic.type_params.len()));
        self.check_class(&erased);
        self.current_generic = saved;
    }

    /// Un module est vérifié une fois, comme une classe portant ses seuls
    /// membres : ce qu'il déclare est contrôlé ; un accès `self.x` à un
    /// membre de la classe qui l'utilisera reste permissif (`self` n'y est
    /// pas une classe connue de la table des symboles).
    pub(crate) fn check_module(&mut self, module: &ModuleDecl) {
        let as_class = ClassDecl {
            name: module.name.clone(),
            extends: None,
            modules: Vec::new(),
            implements: Vec::new(),
            members: module.members.clone(),
            span: module.span.clone(),
            is_struct: false,
            implicit_init: false,
        };
        self.check_class(&as_class);
    }
}
