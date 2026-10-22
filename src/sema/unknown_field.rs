/// Champ inconnu d'une classe ou d'une instance de générique : refusé en
/// écriture comme en lecture — une affectation à un champ inexistant
/// écrivait à l'offset 0 du bloc (SIGSEGV sur une chaîne ou un tableau,
/// champ écrasé sur un objet, voir
/// docs/roadmap.d/sema-affectation-champ-inexistant.md).
use crate::parsing::ast::Type;
use crate::sema::typecheck::{type_class_name, TypeChecker};

impl<'a> TypeChecker<'a> {
    /// Classe ou générique à nommer dans `FieldNotFound` si `obj_ty` n'a pas
    /// de champ `field` ; un nom de méthode ne vaut qu'en lecture. `None` si
    /// le champ existe ou si le type n'est pas vérifiable (`mixed`, classe
    /// opaque, parent inconnu).
    pub(crate) fn unknown_field(&self, obj_ty: &Type, field: &str, write: bool) -> Option<String> {
        if let Type::Generic { name, .. } = obj_ty {
            let generic = self.symbols.lookup_generic(name)?;
            let own = generic.fields.contains_key(field) || (!write && generic.methods.contains_key(field));
            let inherited = match &generic.extends {
                Some(parent) if self.symbols.lookup_class(parent).is_none() => return None,
                Some(parent) => self.has_member(parent, field, write),
                None => false,
            };
            return (!own && !inherited).then(|| name.clone());
        }
        let class = type_class_name(obj_ty)?;
        if self.symbols.lookup_class(&class)?.is_opaque {
            return None;
        }
        (!self.has_member(&class, field, write)).then_some(class)
    }

    fn has_member(&self, class: &str, field: &str, write: bool) -> bool {
        self.symbols.lookup_field_owner(class, field).is_some()
            || (!write && self.symbols.lookup_method_in_chain(class, field).is_some())
    }
}
