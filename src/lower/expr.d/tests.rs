/// Tests unitaires — deux domaines partageant ce fichier (`src/lower/expr.d/`) :
/// - parité statique/sucre pour le type des paramètres
///   (docs/roadmap.d/qualite-parite-sucre-statique-param-types.md) ;
/// - résolution récursive du type d'une indexation chaînée
///   (docs/roadmap.d/langage-index-chaine-sur-map.md).

#[cfg(test)]
mod tests {
    use crate::lower::builder::LowerBuilder;
    use crate::lower::expr::helpers::{param_type_for_call_arg, CallForm, elem_type_after_index, is_map_target, resolve_chained_field_class};
    use crate::ir::module::IrModule;
    use crate::ir::types::IrType;
    use crate::parsing::ast::{Expr, Type};
    use crate::parsing::token::Span;

    fn span() -> Span { Span::new(0, 0) }
    fn ident(name: &str) -> Expr { Expr::Ident(name.to_string(), span()) }
    fn index(object: Expr, idx: Expr) -> Expr {
        Expr::Index { object: Box::new(object), index: Box::new(idx), span: span() }
    }
    fn int_lit(n: i64) -> Expr { Expr::Literal(crate::parsing::ast::Literal::Int(n), span()) }
    fn new_expr(class: &str) -> Expr {
        Expr::New { class: class.to_string(), type_args: vec![], args: vec![], span: span() }
    }

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

    // ── is_map_target / elem_type_after_index (indexation chaînée) ─────────
    // docs/roadmap.d/langage-index-chaine-sur-map.md — `arr[0]["champ"]`
    // retournait silencieusement `null` : `is_map_target` ne reconnaissait
    // `Expr::Ident`/`Expr::Field` comme cible map, jamais un `Expr::Index`
    // imbriqué. Ces tests fixent le comportement à une profondeur
    // ARBITRAIRE, pas seulement le cas à deux niveaux du repro original.

    /// `arr[0]` (un seul niveau) — `is_map_target` sur `Ident` reste géré
    /// par `map_vars`, chemin INCHANGÉ par ce correctif ; vérifié ici pour
    /// fixer la non-régression à côté du nouveau chemin `Index`.
    #[test]
    fn is_map_target_single_level_ident_unaffected() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.map_vars.insert("m".to_string());

        assert!(is_map_target(&builder, &ident("m")));
        assert!(!is_map_target(&builder, &ident("arr")));
    }

    /// Cas exact du repro : `arr[0]["champ"]` — `arr: array<map<string,mixed>>`.
    /// `is_map_target(arr[0])` doit répondre "oui, c'est une map".
    #[test]
    fn is_map_target_depth_2_array_of_map() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        // `array<map<string, mixed>>` : elem_ast_types["arr"] = map<string,mixed>
        builder.elem_ast_types.insert(
            "arr".to_string(),
            Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
        );

        let arr_0 = index(ident("arr"), int_lit(0));
        assert!(is_map_target(&builder, &arr_0), "arr[0] doit être reconnu comme une map");
    }

    /// Profondeur 3 : `matrix[0][1]` — `matrix: array<array<map<string,mixed>>>`.
    /// Vérifie que la récursion pèle correctement DEUX couches array avant
    /// d'atteindre la map, pas seulement une.
    #[test]
    fn is_map_target_depth_3_nested_arrays_of_map() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        // array<array<map<string,mixed>>> : elem_ast_types["matrix"] = array<map<string,mixed>>
        builder.elem_ast_types.insert(
            "matrix".to_string(),
            Type::Array(Box::new(Type::Map(Box::new(Type::String), Box::new(Type::Mixed)))),
        );

        let matrix_0 = index(ident("matrix"), int_lit(0));
        let matrix_0_1 = index(matrix_0, int_lit(1));
        assert!(is_map_target(&builder, &matrix_0_1), "matrix[0][1] doit être reconnu comme une map");
    }

    /// Profondeur 5 — vérifie qu'il n'y a aucune limite de profondeur codée
    /// en dur : la composition récursive doit fonctionner pour un nombre
    /// arbitraire de niveaux, pas seulement 2 ou 3.
    #[test]
    fn elem_type_after_index_depth_5_has_no_hardcoded_limit() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        // array<array<array<array<map<string,mixed>>>>> : 4 couches array
        // avant la map. `elem_ast_types["deep"]` représente déjà le type de
        // `deep[i]` (un niveau d'indexation déjà consommé), donc il ne reste
        // que 3 couches array à peler avant d'atteindre la map pour que le
        // 4ᵉ `[...]` de la chaîne (deep[0][0][0][0]) l'expose.
        let mut ty = Type::Map(Box::new(Type::String), Box::new(Type::Mixed));
        for _ in 0..3 {
            ty = Type::Array(Box::new(ty));
        }
        builder.elem_ast_types.insert("deep".to_string(), ty);

        // deep[0][0][0][0] : 4 index imbriqués sur "deep" (déjà "un niveau
        // dans" via elem_ast_types) doivent atteindre la map.
        let mut expr = ident("deep");
        for _ in 0..4 {
            expr = index(expr, int_lit(0));
        }
        assert!(is_map_target(&builder, &expr), "deep[0][0][0][0] doit être reconnu comme une map à 5 niveaux de profondeur");
    }

    /// Non-régression : une indexation chaînée sur des arrays PURS (aucune
    /// map impliquée) ne doit jamais être classée comme une map.
    #[test]
    fn is_map_target_pure_arrays_never_map_shaped() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        // array<array<int>> : elem_ast_types["nums"] = array<int>
        builder.elem_ast_types.insert(
            "nums".to_string(),
            Type::Array(Box::new(Type::Int)),
        );

        let nums_0 = index(ident("nums"), int_lit(0));
        assert!(!is_map_target(&builder, &nums_0), "nums[0] (array<int>) ne doit jamais être classé comme une map");
    }

    /// `map<K,V>|null` (union) — même dépliage que `map_value_type`
    /// (variables.rs), vérifié ici pour le chemin `container_elem_type`
    /// utilisé par la résolution récursive d'indexation.
    #[test]
    fn elem_type_after_index_unwraps_union_with_null() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.elem_ast_types.insert(
            "arr".to_string(),
            Type::Union(vec![
                Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
                Type::Null,
            ]),
        );

        let arr_0 = index(ident("arr"), int_lit(0));
        assert!(is_map_target(&builder, &arr_0), "arr[0] (map<...>|null) doit être reconnu comme une map");
    }

    /// Expression inconnue (ni `Ident` ni `Index` ni `Field`) — doit
    /// retourner `None`/`false` sans paniquer, pas planter le compilateur
    /// sur une forme d'expression non prévue.
    #[test]
    fn elem_type_after_index_unknown_expr_returns_none_without_panic() {
        let mut module = IrModule::new("test");
        let builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);

        let lit = int_lit(42);
        assert_eq!(elem_type_after_index(&builder, &lit), None);
        assert!(!is_map_target(&builder, &lit));
    }

    // ── resolve_chained_field_class avec Expr::New en base (`use Classe(...).*`) ──
    // `use Classe(...).méthode()`/`.champ` chaîné directement, sans jamais
    // lier l'instance à une variable, n'était reconnu par AUCUN chemin de
    // résolution de classe du lowering (`Expr::Ident`/`SelfExpr`/`ParentExpr`/
    // `Field` seulement) : `resolve_chained_field_class` (et les ~7 autres
    // points d'entrée équivalents dans lower.rs/typeinfer.rs/helpers.rs/
    // assignments.rs/message_gen.rs, tous corrigés ensemble) retombait sur
    // `None`, ou pire, sur le filet de secours "String" pour un appel de
    // méthode (`lower.rs`) — `func_mangled` valait alors `"String_run"` pour
    // `use Thread().run(...)`, un symbole qui n'existe pas, et le codegen
    // (`emit_calls`) ignore silencieusement un appel vers une fonction
    // inconnue au lieu d'échouer : le thread n'était alors JAMAIS lancé,
    // sans la moindre erreur de compilation. Confirmé aussi sur une valeur
    // de retour scalaire (`use Bar().getIt()` valait toujours `0`).

    /// `use Foo().inner.method()` — `inner: Bar`. `resolve_chained_field_class`
    /// doit reconnaître `use Foo()` comme base de classe `Foo` et résoudre le
    /// champ `inner` vers `Bar`, exactement comme si `Foo` avait été assignée
    /// à une variable d'abord.
    #[test]
    fn resolve_chained_field_class_recognizes_new_expr_as_base() {
        let mut module = IrModule::new("test");
        module.class_field_types.insert("Foo".to_string(), vec![("inner".to_string(), Type::Named("Bar".to_string()))]);
        let builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);

        let result = resolve_chained_field_class(&builder, &new_expr("Foo"), "inner");
        assert_eq!(result, Some("Bar".to_string()), "use Foo().inner doit résoudre vers la classe Bar");
    }
}
