/// Tests unitaires — parité statique/sucre pour le type des paramètres
/// (docs/roadmap.d/qualite-parite-sucre-statique-param-types.md).

#[cfg(test)]
mod tests {
    use crate::lower::builder::LowerBuilder;
    use crate::lower::expr::helpers::{param_type_for_call_arg, CallForm};
    use crate::ir::module::IrModule;
    use crate::ir::types::IrType;

    /// `Array::get(arr, idx)` — signature réelle déclarée dans
    /// `src/builtins/array.rs` : `[arr: array<mixed> → Ptr, idx: int → I64]`.
    /// Vérifie que les deux formes d'appel retrouvent le bon type pour
    /// chaque paramètre, exactement le cas dont l'inversion a causé le
    /// SEGFAULT documenté dans `docs/roadmap.d/langage-array-get-display-bug.md`.
    #[test]
    fn static_and_sugar_agree_on_array_get_param_types() {
        let mut module = IrModule::new("test");
        let builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);

        // Forme statique : Array::get(arr, idx) — args[0]=arr, args[1]=idx,
        // positions réelles dans la table (récepteur inclus).
        assert_eq!(param_type_for_call_arg(&builder, "Array_get", 0, CallForm::Static), Some(IrType::Ptr));
        assert_eq!(param_type_for_call_arg(&builder, "Array_get", 1, CallForm::Static), Some(IrType::I64));

        // Forme sucre : arr.get(idx) — args[0]=idx (récepteur implicite,
        // absent de `args`) : doit retrouver le type du paramètre idx (I64),
        // jamais celui du récepteur (Ptr).
        assert_eq!(param_type_for_call_arg(&builder, "Array_get", 0, CallForm::Sugar), Some(IrType::I64));
    }

    /// Propriété générale, pas spécifique à `Array::get` : pour tout builtin
    /// à double forme, l'argument `i` du sucre doit toujours retrouver
    /// exactement ce que l'argument `i + 1` de la forme statique retrouve —
    /// c'est précisément la relation que le bug historique a rompue. Un futur
    /// changement qui romprait à nouveau cette relation (ex. une des deux
    /// branches modifiée sans l'autre) ferait échouer ce test immédiatement,
    /// sans attendre une reproduction par SEGFAULT sur un programme `.oc`.
    #[test]
    fn sugar_offset_is_exactly_one_relative_to_static() {
        let mut module = IrModule::new("test");
        let builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);

        for mangled in ["Array_get", "Array_slice", "Map_get"] {
            for i in 0..2 {
                let sugar = param_type_for_call_arg(&builder, mangled, i, CallForm::Sugar);
                let static_shifted = param_type_for_call_arg(&builder, mangled, i + 1, CallForm::Static);
                assert_eq!(
                    sugar, static_shifted,
                    "{mangled}: sucre[{i}] doit == statique[{}] (sugar={sugar:?}, static_shifted={static_shifted:?})",
                    i + 1
                );
            }
        }
    }

    /// Les tables utilisateur (`fn_param_types`/`module.method_param_types`)
    /// ne déclarent JAMAIS de récepteur implicite — contrairement à
    /// `builtin_method_param_types()`, `i` doit s'y appliquer identiquement
    /// pour les deux formes, sans le décalage `+1` du sucre.
    #[test]
    fn user_function_param_types_never_shift_between_forms() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.fn_param_types.insert("MaClasse_methode".into(), vec![IrType::I64, IrType::F64]);

        assert_eq!(param_type_for_call_arg(&builder, "MaClasse_methode", 0, CallForm::Static), Some(IrType::I64));
        assert_eq!(param_type_for_call_arg(&builder, "MaClasse_methode", 0, CallForm::Sugar), Some(IrType::I64));
        assert_eq!(param_type_for_call_arg(&builder, "MaClasse_methode", 1, CallForm::Static), Some(IrType::F64));
        assert_eq!(param_type_for_call_arg(&builder, "MaClasse_methode", 1, CallForm::Sugar), Some(IrType::F64));
    }

    /// Un `mangled`/index inconnu retourne `None` pour les deux formes, sans
    /// paniquer — le boxing est alors simplement sauté (comportement inchangé
    /// depuis avant l'introduction de ce mécanisme).
    #[test]
    fn unknown_mangled_returns_none_for_both_forms() {
        let mut module = IrModule::new("test");
        let builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);

        assert_eq!(param_type_for_call_arg(&builder, "Inconnu_methode", 0, CallForm::Static), None);
        assert_eq!(param_type_for_call_arg(&builder, "Inconnu_methode", 0, CallForm::Sugar), None);
    }
}
