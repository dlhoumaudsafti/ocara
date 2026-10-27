/// Accès par clé à une valeur `mixed` : `x[k]`, `x.clé` (lecture) et
/// `x[k] = v`, `x.clé = v` (écriture), réécrits en `Convert::mixedGet` /
/// `Convert::mixedSet` (`AstRewrites::mixed_reads`/`mixed_writes`, réinjectés
/// par `core::named_args`) : map ou tableau choisi à l'exécution selon la
/// valeur. Le lowering, qui ne connaît que les types statiques, lisait une
/// map portée par un `mixed` comme un tableau (`null`) et `x.clé` au début
/// du bloc.
///
/// Restent hors réécriture : la variable d'un `on e` sans filtre (ses champs
/// `message`/`code`/`source` passent par `__exc_get_field`) et le corps
/// d'un `generic` vérifié sous sa forme effacée, où `T` vaut `mixed`.

use crate::parsing::ast::{Expr, Type};
use crate::parsing::token::Span;
use crate::sema::named_args::site_key;
use crate::sema::typecheck::TypeChecker;

impl<'a> TypeChecker<'a> {
    fn is_mixed_receiver(&self, object: &Expr, obj_ty: &Type) -> bool {
        *obj_ty == Type::Mixed
            && self.current_generic.is_none()
            && !matches!(object, Expr::Ident(name, _) if self.catch_all_bindings.contains(name))
    }

    /// Lecture `object[…]`/`object.…` (span de l'accès) : notée si `object`
    /// est un `mixed`.
    pub(crate) fn note_mixed_read(&mut self, object: &Expr, obj_ty: &Type, span: &Span) {
        if self.is_mixed_receiver(object, obj_ty) {
            self.rewrites.mixed_reads.insert(site_key(span));
        }
    }

    /// Affectation à `object[…]`/`object.…` (span de la cible) : vrai si
    /// `object` est un `mixed`, l'affectation étant notée.
    pub(crate) fn note_mixed_write(&mut self, object: &Expr, obj_ty: &Type, span: &Span) -> bool {
        let mixed = self.is_mixed_receiver(object, obj_ty);
        if mixed {
            self.rewrites.mixed_writes.insert(site_key(span));
        }
        mixed
    }
}
