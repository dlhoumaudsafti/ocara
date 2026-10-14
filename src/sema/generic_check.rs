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
///
/// Chaque instanciation concrète écrite dans le programme (`use
/// Box<string>(…)`) est ensuite revérifiée avec `T` remplacé : une opération
/// invalide pour ce type (`-` sur une `string`, E59) y est signalée, en
/// nommant l'instanciation — voir docs/roadmap.d/langage-compound-assignment.md.

use crate::parsing::ast::{ClassDecl, GenericDecl, ModuleDecl, Program, Type};
use crate::sema::error::SemaError;
use crate::sema::typecheck::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_generic(&mut self, generic: &GenericDecl) {
        let erased = crate::core::monomorph::erased_generic_class(generic);
        let saved = self.current_generic.replace((generic.name.clone(), generic.type_params.len()));
        self.check_class(&erased);
        self.current_generic = saved;
    }

    /// Revérifie chaque instanciation concrète du programme ; seules les
    /// erreurs propres à l'instanciation (absentes de la vérification
    /// effacée) sont gardées.
    pub(crate) fn check_generic_instances(&mut self, program: &Program) {
        for (name, type_args) in crate::core::monomorph::generic_instantiations(program) {
            let Some(generic) = program.generics.iter().find(|g| g.name == name) else { continue };
            if type_args.len() != generic.type_params.len() || !type_args.iter().all(|t| self.is_concrete(t)) {
                continue;
            }
            let instance = crate::core::monomorph::instantiated_generic_class(generic, &type_args);
            let known: Vec<String> = self.errors.iter().map(|e| format!("{}@{:?}", e.message(), e.span())).collect();
            let errors_before = self.errors.len();
            let warnings_before = self.warnings.len();
            let saved = self.current_generic.replace((generic.name.clone(), generic.type_params.len()));
            let saved_args = self.current_generic_args.replace(type_args.clone());
            self.checked_classes.remove(&generic.name);
            self.check_class(&instance);
            self.current_generic = saved;
            self.current_generic_args = saved_args;
            self.warnings.truncate(warnings_before);
            let label = format!("{}<{}>", name, type_args.iter().map(crate::sema::typecheck::type_name).collect::<Vec<_>>().join(", "));
            let fresh: Vec<SemaError> = self.errors.drain(errors_before..)
                .filter(|e| !known.contains(&format!("{}@{:?}", e.message(), e.span())))
                .map(|e| SemaError::InGenericInstance { instance: label.clone(), message: e.message(), span: e.span().clone() })
                .collect();
            self.errors.extend(fresh);
        }
    }

    /// Type sans paramètre de type libre (`T` d'un autre générique).
    fn is_concrete(&self, ty: &Type) -> bool {
        match ty {
            Type::Named(n) => self.symbols.lookup_class(n).is_some() || self.symbols.lookup_interface(n).is_some() || self.symbols.lookup_enum(n).is_some(),
            Type::Array(t) | Type::Message(t) | Type::Resolvable(t) => self.is_concrete(t),
            Type::Map(k, v) => self.is_concrete(k) && self.is_concrete(v),
            Type::Union(vs) => vs.iter().all(|v| self.is_concrete(v)),
            Type::Generic { args, .. } => args.iter().all(|a| self.is_concrete(a)),
            _ => true,
        }
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
