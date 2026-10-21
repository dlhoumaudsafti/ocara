/// `mixed` utilisé là où une `string` est attendue (déclaration, affectation,
/// argument, `return`) : la valeur est convertie à l'exécution par
/// `Convert::mixedToStr` — une chaîne passe telle quelle, un `int`/`float`/
/// `bool` devient son texte, `null` reste `null`. Sans conversion, un entier
/// logé dans le `mixed` (`req.param("id")` d'une route `<id:int>`) était lu
/// comme une adresse de chaîne : SIGSEGV. Voir
/// docs/roadmap.d/langage-mixed-vers-string-non-converti.md.
///
/// Tout autre type concret attendu (`int`, classe, tableau…) : le type de la
/// valeur est vérifié à l'exécution par `Convert::mixedAs`.
///
/// La sema note l'expression ; `core::named_args::rewrite_program`
/// l'enveloppe ensuite dans l'appel de conversion.

use crate::parsing::ast::{Expr, Type};
use crate::sema::named_args::site_key;
use crate::sema::typecheck::TypeChecker;

impl<'a> TypeChecker<'a> {
    /// `mixed` rangé dans un type concret : converti en `string`, ou vérifié
    /// à l'exécution pour un autre type (`Convert::mixedAs`, une valeur d'un
    /// autre type lève une `ConvertException` au lieu de planter plus loin).
    pub(crate) fn note_mixed_conversion(&mut self, value: &Expr, found: &Type, expected: &Type) {
        if !matches!(found, Type::Mixed) {
            return;
        }
        let key = (site_key(value.span()), std::mem::discriminant(value));
        if matches!(expected, Type::String) {
            self.rewrites.mixed_to_string.insert(key);
        } else if let Some(descriptor) = self.runtime_type_descriptor(expected) {
            self.rewrites.mixed_checks.insert(key, descriptor);
        }
    }

    /// Nom de type compris par `Convert::mixedAs` ; `None` : pas de contrôle
    /// possible (union, `mixed`, paramètre de type…).
    fn runtime_type_descriptor(&self, ty: &Type) -> Option<String> {
        Some(match ty {
            Type::Int => "int".into(),
            Type::Float => "float".into(),
            Type::Bool => "bool".into(),
            Type::Array(_) => "array".into(),
            Type::Map(..) => "map".into(),
            Type::Function { .. } => "function".into(),
            Type::Named(name) if self.symbols.lookup_enum(name).is_some() => "int".into(),
            Type::Named(name) if self.declared_by_program(name) => format!("class:{}", name),
            Type::Generic { name, args } if self.current_generic.is_none() =>
                format!("class:{}", crate::core::monomorph::monomorphized_name(name, args)),
            _ => return None,
        })
    }

    /// Classe ou interface du programme (instances `TAG_OBJECT` connues de
    /// la table des classes du runtime).
    fn declared_by_program(&self, name: &str) -> bool {
        self.program.is_some_and(|p| p.classes.iter().any(|c| c.name == name) || p.interfaces.iter().any(|i| i.name == name))
    }
}
