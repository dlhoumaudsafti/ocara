/// `mixed` utilisé là où une `string` est attendue (déclaration, affectation,
/// argument, `return`) : la valeur est convertie à l'exécution par
/// `Convert::mixedToStr` — une chaîne passe telle quelle, un `int`/`float`/
/// `bool` devient son texte, `null` reste `null`. Sans conversion, un entier
/// logé dans le `mixed` (`req.param("id")` d'une route `<id:int>`) était lu
/// comme une adresse de chaîne : SIGSEGV. Voir
/// docs/roadmap.d/langage-mixed-vers-string-non-converti.md.
///
/// La sema note l'expression ; `core::named_args::rewrite_program`
/// l'enveloppe ensuite dans l'appel de conversion.

use crate::parsing::ast::{Expr, Type};
use crate::sema::named_args::site_key;
use crate::sema::typecheck::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn note_mixed_to_string(&mut self, value: &Expr, found: &Type, expected: &Type) {
        if matches!(found, Type::Mixed) && matches!(expected, Type::String) {
            self.rewrites.mixed_to_string.insert((site_key(value.span()), std::mem::discriminant(value)));
        }
    }
}
