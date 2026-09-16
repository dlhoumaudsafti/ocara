# `Array::get(arr, i)` mal affiché quand utilisé directement comme argument

## ✅ Corrigé

`expr_ir_type` (`src/lower/expr.d/typeinfer.rs`, `Expr::StaticCall`) consulte désormais `builder.elem_types` de l'objet en premier argument pour `Array::get`/`first`/`last`/`pop` et `Map::get`, AVANT de retomber sur le `Ptr` générique de `fn_ret_types` — même logique que `Expr::Index` pour un accès direct (`arr[i]`). Vérifié : `IO::writeln(Array::get(arr, i))`/`IO::writeln(Map::get(m, k))` affichent désormais correctement `0`/`1.5`/`false` (au lieu de `null`) sur un conteneur à élément concret. Nouveau test dédié : `examples/tests/43_array_map_get_concrete_typeTest.oc` (13 assertions) + `make regression` repassé au vert.

## Découverte originale (pour référence)

Trouvé en vérifiant `Array::fromMessage` (voir docs/roadmap.d/langage-emit-iterable.md) — bug **pré-existant, sans rapport avec `emit`/les générateurs** : reproductible avec un simple littéral `array<int>`, aucun générateur impliqué.

```ocara
var values:array<int> = [0, 1, 2, 3]
IO::writeln(Array::get(values, 0))   // affichait "null" avant le correctif
```

`program.rs` enregistre `fn_ret_types.insert("Array_get", IrType::Ptr)` — correct pour `array<mixed>` (éléments déjà boxés), mais faux pour un `array<T>` à élément concret : la valeur brute `0` était interprétée comme un pointeur nul par le dispatch typé de `IO::write`/`IO::writeln`.

## ✅ Ambiguïté `0`/`null` dans un `mixed` — corrigé

`UnitTest::assertEquals(0, Array::get(arr, i))` (paramètres `mixed`) affichait `"null == null"` dans son log, alors que la comparaison elle-même restait correcte (0 FAIL). Cause : un paramètre `mixed` fait passer la valeur par le boxing générique — et `box_int_if_needed` (`runtime/src/lib.rs`) ne boxait **jamais** un petit entier (optimisation volontaire, `n < 0x10000` restait brut). Un entier brut `0` logé dans un `mixed` était donc **structurellement indiscernable** de `null` (0 = pointeur nul), et `val_to_string`/`__val_to_str` commencent explicitement par `if val == 0 { return "null" }`.

Corrigé en boxant spécifiquement `0` (même mécanisme que pour un grand entier, magnitude mise à part) : `box_int_if_needed` boxe désormais `n != 0 && n < 0x10000` OU `n == 0`. Tous les consommateurs (`get_value_type`, `__is_int`/`__is_null`, `cmp_primitive`/`__cmp_eq_strict`, `val_to_string`, `ut_val_to_display`, `UnitTest_assertNull`/`assertEmpty`/`assertNotEmpty`, `value_to_json`) branchaient déjà correctement sur le tag de boxing AVANT de retomber sur `val == 0` — ce correctif d'un point profite donc à tous sans changement supplémentaire. 18 nouvelles assertions dans `examples/tests/37_mixed_large_intTest.oc`.

## ✅ Bug distinct découvert EN VÉRIFIANT ce correctif — décalage d'index sucre/statique, corrigé

`make regression` a régressé (589 au lieu de 609 attendus, 0 FAIL rapporté — des assertions entières manquaient) après le correctif `0`/`null` ci-dessus : `examples/tests/35_var_auto_freeTest.oc` (`escapesViaConstructorAndArrayTest`, 500 objets `Holder` dans un `array<Holder>`, relus par index) **SEGFAULT** de façon reproductible. Isolé par reproduction minimale (`array<Holder>`, propriété `int`, `acc.get(0)` puis lecture du champ) : le crash survient uniquement sur l'INDEX `0`, en forme SUCRE (`acc.get(j)`), jamais en forme statique (`Array::get(acc, j)`).

Root cause, dans `src/lower/expr.d/lower.rs` (branche `Expr::Call{callee: Field}`, l'appel sucré) : la décision de boxer un argument (`box_arg_for_mixed_param`) consultait `param_type_for_call_arg(builder, func_mangled, i)` avec `i` = position dans les arguments EXPLICITES du sucre (`args`, qui ne contient JAMAIS le récepteur). Mais la table de signatures des builtins à double forme (`builtin_method_param_types()`, construite depuis `src/builtins/array.rs` etc.) déclare chaque méthode comme sa forme STATIQUE — récepteur INCLUS en position 0 (`Array::get(arr, idx)` → `params = [arr, idx]`). Résultat : `idx` (position 0 côté sucre) se voyait attribuer le type du RÉCEPTEUR (`arr`, toujours `mixed`/`Ptr`) au lieu de son propre type déclaré (`int`, position 1 réelle) — décalage d'index d'une position, présent depuis l'introduction de `box_arg_for_mixed_param`, mais **invisible tant que `box_int_if_needed` ne boxait jamais un entier en dessous du seuil pointeur** (l'index boxé à tort redevenait la valeur brute une fois déboxé). Le correctif `0`/`null` ci-dessus a rendu ce décalage visible pour la première fois : `arr.get(0)` boxait l'index `0` à tort, `__array_get` recevait un pointeur boxé arbitraire au lieu de l'entier `0` — hors-borne, retournait `null` silencieusement (élément concret) ou faisait planter le programme si le résultat était ensuite déréférencé sans vérification (accès de champ sur un objet, cas de `Holder`/`35_var_auto_freeTest.oc`).

Corrigé par une nouvelle fonction dédiée, `param_type_for_sugar_call_arg` (`src/lower/expr.d/helpers.rs`), qui décale l'index de `+1` UNIQUEMENT quand la table `builtin_method_param_types()` est la source (les tables utilisateur, `fn_param_types`/`module.method_param_types`, ne déclarent jamais de récepteur implicite et n'ont donc pas besoin de ce décalage). Seul appelant concerné : la branche sucre de `Expr::Call{callee: Field}` — les formes statique et constructeur utilisaient déjà la bonne table/le bon index. Ce bug touchait potentiellement TOUT builtin à double forme avec un paramètre scalaire à une position différente de celle du récepteur (`get`/`slice`/`set`...) — resté invisible ailleurs par pure coïncidence (la plupart des positions concurrentes sont `Ptr` des deux côtés). Nouvelles assertions dédiées dans `examples/tests/38_mixed_call_arg_boxingTest.oc` (6 assertions : `Array::get`/`slice` sucre, index `0`, dont un cas objet reproduisant le SEGFAULT original).

## Priorité

✅ Terminé — les trois volets (affichage `Array::get`/`Map::get`, ambiguïté `0`/`null`, décalage d'index sucre/statique) sont corrigés et vérifiés (`make regression` : 610 PASS, 0 FAIL, 0 ERREUR).
