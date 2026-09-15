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

## ⚠️ Bug distinct découvert en corrigeant celui-ci — PAS corrigé, plus profond

`UnitTest::assertEquals(0, Array::get(arr, i))` (paramètres `mixed`) affiche toujours `"null == null"` dans son log, alors que la comparaison elle-même reste correcte (0 FAIL). Cause : contrairement à `IO::writeln` (qui peut maintenant dispatcher sur le type CONCRET grâce au correctif ci-dessus, sans jamais boxer), un paramètre `mixed` fait passer la valeur par le boxing générique — et `box_int_if_needed` (`runtime/src/lib.rs`) ne boxe **jamais** un petit entier (optimisation volontaire, `n < 0x10000` reste brut). Un entier brut `0` logé dans un `mixed` est donc **structurellement indiscernable** de `null` (0 = pointeur nul), et `val_to_string`/`__val_to_str` (`runtime/src/lib.rs`) commencent explicitement par `if val == 0 { return "null" }`.

Ce n'est pas un bug de dispatch comme celui corrigé ci-dessus : c'est une **ambiguïté de représentation** dans le schéma de boxing `mixed` lui-même (`0` = à la fois "null" et "entier zéro non boxé"), qui affecte potentiellement tout code générique consommant un `mixed` (affichage, JSON/YAML, comparaisons `is_int`/`is_null`...). Une vraie correction demanderait de revoir comment `null` est représenté (actuellement confondu avec l'entier 0 partout) — changement de fond, hors de proportion pour ce correctif ponctuel. À qualifier séparément si ça devient gênant en pratique.

## Priorité

Terminé pour la partie corrigée. La partie non corrigée (ambiguïté `0`/`null` dans `mixed`) mérite sa propre fiche si elle doit être traitée un jour — Structurel, touche une zone sensible (boxing/représentation), à ne pas improviser.
