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

    // ── Appel chaîné sur le résultat d'une fonction LIBRE ───────────────────────
    // docs/roadmap.d/langage-chained-call-on-free-function-result.md — même
    // famille que le cas `use Classe(...).*` ci-dessus, troisième
    // déclencheur (`maFonction(...).methode()`), jamais couvert par ce
    // correctif-là. Root cause confirmée par `ocara build --dump` (HIR) AVANT
    // correctif : le site d'appel manglait vers `"_method_<methode>"` (sans
    // préfixe de classe, symbole inexistant) plutôt que `"Circle_shapeName"`.
    // Pipeline complet (parse → lower_program) utilisé ici plutôt qu'un
    // `LowerBuilder` nu : le correctif vit dans `lower_expr` lui-même (le
    // bras `Expr::Field` de `Expr::Call`), pas dans une fonction utilitaire
    // isolée comme `resolve_chained_field_class` ci-dessus — inspecter le
    // HIR réellement généré est la façon la plus directe de vérifier le
    // symptôme observable (le `Inst::Call.func` au site d'appel).
    use crate::lower::builder::program::lower_program;
    use crate::ir::inst::Inst;
    use crate::parsing::lexer::Lexer;
    use crate::parsing::parser::Parser;

    fn lower_src(src: &str) -> IrModule {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        let program = Parser::new(tokens).parse_program().expect("parse");
        lower_program(&program, "<test>")
    }

    /// Cherche, parmi TOUS les `Inst::Call` de `func_name`, celui dont la
    /// cible correspond au SUFFIXE donné (`"_shapeName"`, par exemple) —
    /// utilisé pour trouver le second appel d'une chaîne (`pickCircle(0).shapeName()`
    /// émet D'ABORD un appel à `"pickCircle"`, PUIS l'appel de méthode qui
    /// nous intéresse réellement ici).
    fn find_call_ending_with<'a>(module: &'a IrModule, func_name: &str, suffix: &str) -> &'a str {
        let func = module.functions.iter().find(|f| f.name == func_name)
            .unwrap_or_else(|| panic!("function '{}' not found", func_name));
        func.blocks.iter().flat_map(|b| &b.insts)
            .find_map(|i| match i {
                Inst::Call { func, .. } if func.ends_with(suffix) => Some(func.as_str()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no call ending with '{}' found in '{}'", suffix, func_name))
    }

    /// `IrModule::func_ret_class` doit connaître le nom de classe RÉEL d'une
    /// fonction libre retournant un type nommé — la donnée qui manquait
    /// avant le correctif.
    #[test]
    fn func_ret_class_knows_user_class_return_type() {
        let module = lower_src(
            "class Circle {\n\
                 public method shapeName(): string { return \"circle\" }\n\
             }\n\
             function pickCircle(kind:int): Circle {\n\
                 return use Circle(2.0)\n\
             }\n\
             function main(): int {\n\
                 return 0\n\
             }\n",
        );
        assert_eq!(module.func_ret_class.get("pickCircle"), Some(&"Circle".to_string()));
    }

    /// Même chose pour les familles builtin `array`/`map`/`string` — voir la
    /// remarque de `IrModule::func_ret_class` : sans ce cas, `maFonction().len()`
    /// (fonction libre retournant un tableau) échouait exactement de la même
    /// façon qu'une classe utilisateur.
    #[test]
    fn func_ret_class_knows_builtin_container_return_types() {
        let module = lower_src(
            "function makeNames(): array<string> {\n\
                 return [\"a\", \"b\"]\n\
             }\n\
             function makeLookup(): map<string, int> {\n\
                 return {\"a\": 1}\n\
             }\n\
             function makeGreeting(): string {\n\
                 return \"hi\"\n\
             }\n\
             function main(): int {\n\
                 return 0\n\
             }\n",
        );
        assert_eq!(module.func_ret_class.get("makeNames"), Some(&"Array".to_string()));
        assert_eq!(module.func_ret_class.get("makeLookup"), Some(&"Map".to_string()));
        assert_eq!(module.func_ret_class.get("makeGreeting"), Some(&"String".to_string()));
    }

    /// Le cas exact du ticket, vérifié au niveau HIR : `pickCircle(0).shapeName()`
    /// doit mangler vers `"Circle_shapeName"`, jamais `"_method_shapeName"`
    /// (le symbole inexistant émis avant le correctif).
    #[test]
    fn chained_call_on_free_function_result_mangles_to_the_real_class_method() {
        let module = lower_src(
            "class Circle {\n\
                 public method shapeName(): string { return \"circle\" }\n\
             }\n\
             function pickCircle(kind:int): Circle {\n\
                 return use Circle(2.0)\n\
             }\n\
             function main(): int {\n\
                 var s:string = pickCircle(0).shapeName()\n\
                 return 0\n\
             }\n",
        );
        assert_eq!(find_call_ending_with(&module, "main", "shapeName"), "Circle_shapeName");
    }

    /// Non-régression : une fonction libre qui NE retourne PAS un type avec
    /// méthodes (`int`) ne doit jamais apparaître dans `func_ret_class`.
    #[test]
    fn func_ret_class_does_not_capture_primitive_return_types() {
        let module = lower_src(
            "function computeSum(a:int, b:int): int { return a + b }\n\
             function main(): int { return 0 }\n",
        );
        assert_eq!(module.func_ret_class.get("computeSum"), None);
    }

    // ── Appel chaîné à partir de 3 niveaux (`a.b().c().d()`) ────────────────────
    // docs/roadmap.d/langage-chained-call-depth-limit.md — toute la famille de
    // résolution "classe du récepteur d'un appel/accès chaîné" ne recursait
    // qu'un seul niveau, et supposait en plus, à tort, qu'une méthode
    // retournant un pointeur retournait la MÊME classe que son récepteur.
    // `w.getCircle().shapeName()` (2 niveaux) fonctionnait déjà ; ajouter UN
    // niveau de plus (`.upper()`, 3 niveaux) manglait vers `"_method_upper"`
    // (symbole inexistant) au lieu de `"String_upper"`. Corrigé par une seule
    // fonction récursive partagée (`resolve_receiver_class`), qui s'appuie sur
    // `IrModule::method_ret_class` — absent avant ce correctif : une méthode
    // de classe ORDINAIRE (contrairement à une méthode d'interface ou une
    // fonction libre) n'avait AUCUNE entrée retraçant son type de retour réel
    // en tant que CLASSE.

    /// `IrModule::method_ret_class` doit connaître le nom de classe RÉEL du
    /// retour d'une méthode d'instance — la donnée qui manquait avant le
    /// correctif (seules les méthodes d'INTERFACE et les fonctions libres
    /// avaient un équivalent).
    #[test]
    fn method_ret_class_knows_user_class_return_type() {
        let module = lower_src(
            "class Circle {\n\
                 public method shapeName(): string { return \"circle\" }\n\
             }\n\
             class Wrapper {\n\
                 public property inner:Circle\n\
                 init(c:Circle) { self.inner = c }\n\
                 public method getCircle(): Circle { return self.inner }\n\
             }\n\
             function main(): int { return 0 }\n",
        );
        assert_eq!(module.method_ret_class.get("Wrapper_getCircle"), Some(&"Circle".to_string()));
    }

    /// Non-régression : une méthode qui NE retourne PAS un type avec méthodes
    /// (`int`) ne doit jamais apparaître dans `method_ret_class`.
    #[test]
    fn method_ret_class_does_not_capture_primitive_return_types() {
        let module = lower_src(
            "class Counter {\n\
                 public method value(): int { return 42 }\n\
             }\n\
             function main(): int { return 0 }\n",
        );
        assert_eq!(module.method_ret_class.get("Counter_value"), None);
    }

    /// Une méthode héritée (non surchargée) reste résolvable pour le
    /// chaînage sur une INSTANCE DE LA SOUS-CLASSE — même propagation que
    /// `fn_ret_types` pour l'héritage (voir `lower_program`).
    #[test]
    fn method_ret_class_propagates_through_inheritance() {
        let module = lower_src(
            "class Circle {\n\
                 public method shapeName(): string { return \"circle\" }\n\
             }\n\
             class Base {\n\
                 public method getCircle(): Circle { return use Circle(2.0) }\n\
             }\n\
             class Derived extends Base {\n\
             }\n\
             function main(): int { return 0 }\n",
        );
        assert_eq!(module.method_ret_class.get("Derived_getCircle"), Some(&"Circle".to_string()));
    }

    /// Le cas EXACT du ticket, vérifié au niveau HIR : `w.getCircle().shapeName().upper()`
    /// (3 niveaux, DEUX classes différentes enchaînées) doit mangler le
    /// TROISIÈME appel vers `"String_upper"`, jamais `"_method_upper"` (le
    /// symbole inexistant émis avant le correctif — l'ancienne heuristique
    /// supposait à tort que `getCircle()` retournait `Wrapper`, pas `Circle`).
    #[test]
    fn chained_method_call_resolves_the_real_class_at_three_levels() {
        let module = lower_src(
            "class Circle {\n\
                 public method shapeName(): string { return \"circle\" }\n\
             }\n\
             class Wrapper {\n\
                 public property inner:Circle\n\
                 init(c:Circle) { self.inner = c }\n\
                 public method getCircle(): Circle { return self.inner }\n\
             }\n\
             function main(): int {\n\
                 var w:Wrapper = use Wrapper(use Circle(2.0))\n\
                 var s:string = w.getCircle().shapeName().upper()\n\
                 return 0\n\
             }\n",
        );
        assert_eq!(find_call_ending_with(&module, "main", "shapeName"), "Circle_shapeName");
        assert_eq!(find_call_ending_with(&module, "main", "upper"), "String_upper");
    }

    /// Profondeur 4 : vérifie que la récursion est vraiment non bornée, pas
    /// juste étendue à 3 niveaux (`Level1` -> `Level2` -> `Level3` -> `string`
    /// -> `String::upper`), trois classes utilisateur DIFFÉRENTES enchaînées.
    #[test]
    fn chained_method_call_resolves_at_four_levels() {
        let module = lower_src(
            "class Level3 {\n\
                 public method label(): string { return \"deep\" }\n\
             }\n\
             class Level2 {\n\
                 public method next(): Level3 { return use Level3() }\n\
             }\n\
             class Level1 {\n\
                 public method next(): Level2 { return use Level2() }\n\
             }\n\
             function main(): int {\n\
                 var l1:Level1 = use Level1()\n\
                 var s:string = l1.next().next().label().upper()\n\
                 return 0\n\
             }\n",
        );
        assert_eq!(find_call_ending_with(&module, "main", "label"), "Level3_label");
        assert_eq!(find_call_ending_with(&module, "main", "upper"), "String_upper");
    }

    // ── Appel D'INSTANCE `async` (`obj.methode()`) ──────────────────────────────
    // docs/roadmap.d/langage-async-instance-method-dispatch-broken.md — le site
    // d'appel du sucre d'instance n'a JAMAIS empaqueté `self`+args dans un
    // environnement heap ni spawné de tâche, contrairement aux fonctions
    // libres et aux appels statiques : il appelait TOUJOURS `call_target`
    // directement et de façon SYNCHRONE, renvoyant sa vraie valeur comme si
    // c'était déjà un task handle (SIGSEGV confirmé par reproduction+gdb :
    // `resolve` déréférençait ensuite cette valeur comme un pointeur de
    // handle). Corrigé en ajoutant le même mécanisme d'empaquetage/spawn déjà
    // en place pour les deux autres formes d'appel — AUCUN second mécanisme
    // de dispatch : le dispatcher synchrone déjà existant (héritage de classe
    // OU interface) est réutilisé tel quel, seul SON PROPRE wrapper async a
    // besoin d'exister.

    /// Renvoie `true` si `func_name` contient un `Inst::Call` vers `target`.
    fn calls(module: &IrModule, func_name: &str, target: &str) -> bool {
        let func = module.functions.iter().find(|f| f.name == func_name)
            .unwrap_or_else(|| panic!("function '{}' not found", func_name));
        func.blocks.iter().flat_map(|b| &b.insts)
            .any(|i| matches!(i, Inst::Call { func, .. } if func == target))
    }

    /// Renvoie le nom de fonction ciblé par le PREMIER `Inst::FuncAddr` de
    /// `func_name` — utilisé pour vérifier QUEL wrapper un site d'appel
    /// spawn réellement (`__task_spawn(func_addr, env_ptr)` prend l'adresse
    /// du wrapper via `FuncAddr`, jamais son nom littéral dans `Inst::Call`).
    fn func_addr_target<'a>(module: &'a IrModule, func_name: &str) -> &'a str {
        let func = module.functions.iter().find(|f| f.name == func_name)
            .unwrap_or_else(|| panic!("function '{}' not found", func_name));
        func.blocks.iter().flat_map(|b| &b.insts)
            .find_map(|i| match i { Inst::FuncAddr { func, .. } => Some(func.as_str()), _ => None })
            .unwrap_or_else(|| panic!("no FuncAddr found in '{}'", func_name))
    }

    /// Le cas exact du ticket : une classe concrète ordinaire, aucun
    /// héritage, aucune interface. `main` doit spawn une tâche
    /// (`__task_spawn`, via l'adresse de `__async_wrap_DoublingFetcher_fetch`),
    /// jamais appeler `DoublingFetcher_fetch` directement.
    #[test]
    fn async_instance_call_on_plain_class_spawns_a_task() {
        let module = lower_src(
            "class DoublingFetcher {\n\
                 public async method fetch(n:int): int { return n * 2 }\n\
             }\n\
             function main(): int {\n\
                 var f:DoublingFetcher = use DoublingFetcher()\n\
                 var t:int = f.fetch(21)\n\
                 return 0\n\
             }\n",
        );
        assert!(calls(&module, "main", "__task_spawn"), "must spawn a task, not call the method directly");
        assert!(!calls(&module, "main", "DoublingFetcher_fetch"), "must never call the sync method body directly from the call site");
        assert_eq!(func_addr_target(&module, "main"), "__async_wrap_DoublingFetcher_fetch");
        // Le wrapper lui-même doit exister et appeler la vraie méthode.
        assert!(calls(&module, "__async_wrap_DoublingFetcher_fetch", "DoublingFetcher_fetch"));
    }

    /// Non-régression : une méthode D'INSTANCE non-async doit continuer à
    /// être appelée directement, jamais spawnée.
    #[test]
    fn non_async_instance_call_is_never_spawned() {
        let module = lower_src(
            "class Circle {\n\
                 public method shapeName(): string { return \"circle\" }\n\
             }\n\
             function main(): int {\n\
                 var c:Circle = use Circle()\n\
                 var s:string = c.shapeName()\n\
                 return 0\n\
             }\n",
        );
        assert!(calls(&module, "main", "Circle_shapeName"));
        assert!(!calls(&module, "main", "__task_spawn"));
    }

    /// Sous-classe qui surcharge une méthode `async` héritée — dispatch par
    /// identité de classe (`__dispatch_Animal_soundCode`, mécanisme
    /// préexistant, voir `class_dispatch.rs`), toujours 100% synchrone :
    /// c'est SON wrapper async à lui qui doit être spawné, et LUI doit
    /// appeler le dispatcher (qui choisira `Dog_soundCode` à l'exécution).
    #[test]
    fn async_instance_call_through_class_hierarchy_dispatcher_spawns_the_dispatchers_wrapper() {
        let module = lower_src(
            "class Animal {\n\
                 public async method soundCode(): int { return 0 }\n\
             }\n\
             class Dog extends Animal {\n\
                 public async method soundCode(): int { return 7 }\n\
             }\n\
             function main(): int {\n\
                 var a:Animal = use Dog()\n\
                 var t:int = a.soundCode()\n\
                 return 0\n\
             }\n",
        );
        assert!(calls(&module, "main", "__task_spawn"));
        assert_eq!(func_addr_target(&module, "main"), "__async_wrap___dispatch_Animal_soundCode");
        // Le wrapper du dispatcher appelle le dispatcher (synchrone), jamais
        // directement une implémentation concrète.
        assert!(calls(&module, "__async_wrap___dispatch_Animal_soundCode", "__dispatch_Animal_soundCode"));
        // Le dispatcher lui-même reste 100% synchrone et choisit par
        // `__class_id` — non-régression du mécanisme déjà existant.
        assert!(calls(&module, "__dispatch_Animal_soundCode", "Dog_soundCode"));
        assert!(calls(&module, "__dispatch_Animal_soundCode", "Animal_soundCode"));
    }

    /// Interface `wiring`dont le contrat est une méthode D'INSTANCE
    /// `async` — le cas qui a mené à la découverte de ce bug (voir le
    /// ticket). Le dispatcher d'interface (`Repo_fetchCode`, préexistant,
    /// voir `interfaces.rs`) reste 100% synchrone ; c'est SON wrapper async
    /// qui doit être spawné.
    #[test]
    fn async_instance_call_through_interface_wiring_spawns_the_interface_dispatchers_wrapper() {
        let module = lower_src(
            "interface Repo {\n\
                 async method fetchCode(): int\n\
                 wiring ConcreteRepo\n\
             }\n\
             class ConcreteRepo implements Repo {\n\
                 public async method fetchCode(): int { return 99 }\n\
             }\n\
             function main(): int {\n\
                 var r:Repo = use Repo()\n\
                 var t:int = r.fetchCode()\n\
                 return 0\n\
             }\n",
        );
        assert!(calls(&module, "main", "__task_spawn"));
        assert_eq!(func_addr_target(&module, "main"), "__async_wrap_Repo_fetchCode");
        assert!(calls(&module, "__async_wrap_Repo_fetchCode", "Repo_fetchCode"));
        assert!(calls(&module, "Repo_fetchCode", "ConcreteRepo_fetchCode"));
    }

    /// Type concret choisi UNIQUEMENT à l'exécution (paramètre de fonction)
    /// derrière un récepteur typé par une interface — le seul cas où un vrai
    /// dispatch par identité de classe est incontournable, `wiring` ne
    /// résolvant jamais ce genre de binding (toujours un type concret connu
    /// statiquement). Deux implémenteurs, même interface : le dispatcher
    /// doit connaître les deux, son wrapper async doit être spawné quel que
    /// soit le récepteur réel.
    #[test]
    fn async_instance_call_through_runtime_determined_interface_type_spawns_the_shared_dispatcher_wrapper() {
        let module = lower_src(
            "interface Talker {\n\
                 async method speakCode(): int\n\
             }\n\
             class Cat implements Talker {\n\
                 public async method speakCode(): int { return 1 }\n\
             }\n\
             class Robot implements Talker {\n\
                 public async method speakCode(): int { return 2 }\n\
             }\n\
             function pick(kind:int): Talker {\n\
                 if kind equal 0 {\n\
                     return use Cat()\n\
                 } else {\n\
                     return use Robot()\n\
                 }\n\
             }\n\
             function main(): int {\n\
                 var choice:Talker = pick(1)\n\
                 var t:int = choice.speakCode()\n\
                 return 0\n\
             }\n",
        );
        assert!(calls(&module, "main", "__task_spawn"));
        assert_eq!(func_addr_target(&module, "main"), "__async_wrap_Talker_speakCode");
        assert!(calls(&module, "Talker_speakCode", "Cat_speakCode"));
        assert!(calls(&module, "Talker_speakCode", "Robot_speakCode"));
    }
}
