/// Notation pointée sur une map : `truc.A` ≡ `truc["A"]` — voir
/// `docs/EBNF.md` §7.4.
///
/// Résolue sur le type statique du receveur : une `map<string, V>` (ou
/// `map<string, V>|null`) lue, écrite, incrémentée par `.clé` est réécrite
/// en accès indexé à clé littérale (`AstRewrites::map_dots`, réinjecté par
/// `core::named_args`) — le lowering ne voit jamais la forme pointée, et le
/// chemin de `truc["A"]` reste inchangé. `truc.size()` reste un appel de
/// méthode : seul un accès sans appel désigne une clé.

use crate::parsing::ast::{Expr, Literal, Type};
use crate::parsing::token::Span;
use crate::sema::error::SemaError;
use crate::sema::index::Target;
use crate::sema::named_args::site_key;
use crate::sema::typecheck::{type_name, types_compat, TypeChecker};

/// Types clé/valeur de `ty` s'il désigne une map, éventuellement nulle.
fn map_types(ty: &Type) -> Option<(&Type, &Type)> {
    match ty {
        Type::Map(k, v) => Some((k, v)),
        Type::Union(variants) => {
            let mut maps = variants.iter().filter(|v| **v != Type::Null);
            match (maps.next(), maps.next()) {
                (Some(Type::Map(k, v)), None) => Some((k, v)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Clé utilisable après un point : `^[a-zA-Z_][a-zA-Z0-9_]*$` (un
/// `Identifier` admet aussi les lettres Unicode, refusées ici).
pub fn is_dot_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl<'a> TypeChecker<'a> {
    /// `object.field` sans appel, receveur de type `obj_ty` : `None` si ce
    /// n'est pas une map (champ ordinaire) ; sinon le type de `object["field"]`
    /// (type par clé compris), l'accès étant enregistré pour réécriture.
    pub(crate) fn resolve_map_dot(&mut self, object: &Expr, obj_ty: &Type, field: &str, span: &Span) -> Option<Type> {
        let (key_ty, value_ty) = map_types(obj_ty)?;
        let value_ty = value_ty.clone();
        if !types_compat(&Type::String, key_ty, self.symbols) {
            self.errors.push(SemaError::MapDotKeyType {
                key_type: type_name(key_ty),
                field:    field.to_string(),
                span:     self.with_runtime_ctx(span),
            });
            return Some(value_ty);
        }
        if !is_dot_key(field) {
            self.errors.push(SemaError::MapDotInvalidKey { field: field.to_string(), span: self.with_runtime_ctx(span) });
            return Some(value_ty);
        }
        // Instanciation d'un `generic` : le corps est partagé par toutes les
        // instanciations, la réécriture ne peut pas dépendre de l'une d'elles.
        if self.current_generic_args.is_some() && !self.rewrites.map_dots.contains(&site_key(span)) {
            self.errors.push(SemaError::MapDotOnTypeParam { field: field.to_string(), span: self.with_runtime_ctx(span) });
            return Some(value_ty);
        }
        self.rewrites.map_dots.insert(site_key(span));
        let key = Expr::Literal(Literal::String(field.to_string()), span.clone());
        let ty = self.map_key_type(object, &key).unwrap_or(value_ty);
        self.record(span, field, Target::MapKey, &ty);
        Some(ty)
    }

    /// Clés connues de `object` (map initialisée par un littéral) utilisables
    /// après un point, triées — proposées par la complétion de `truc.`.
    pub(crate) fn dot_keys(&self, object: &Expr) -> Vec<(String, Type)> {
        let Expr::Ident(name, _) = object else { return Vec::new() };
        let mut keys: Vec<(String, Type)> = self.map_keys.get(name)
            .map(|keys| keys.iter().filter(|(k, _)| is_dot_key(k)).map(|(k, t)| (k.clone(), t.clone())).collect())
            .unwrap_or_default();
        keys.sort_by(|a, b| a.0.cmp(&b.0));
        keys
    }

    /// Valeur affectée à un élément de tableau ou à une clé de map
    /// (`truc["A"] = v`, `truc.A = v`, `arr[i] = v`) : elle doit convenir au
    /// type de l'emplacement.
    pub(crate) fn check_slot_assign(&mut self, val_ty: &Type, slot_ty: &Type, span: &Span) {
        if !types_compat(val_ty, slot_ty, self.symbols) {
            self.errors.push(SemaError::TypeMismatch {
                expected: type_name(slot_ty),
                found:    type_name(val_ty),
                span:     span.clone(),
            });
        }
    }
}
