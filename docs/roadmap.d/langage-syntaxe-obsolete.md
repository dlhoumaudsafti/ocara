# Syntaxe obsolète et décalages doc/exemples

## ✅ Corrigé

- **`T[]` au lieu de `array<T>`** : corrigé dans tous les fichiers concernés — `examples/generics/List.oc`, `examples/generics/test_syntax.oc`, `examples/runtime/variables.oc`, `examples/tests/{07_loops,08_arrays,16_types,19_break_continue}Test.oc` (`int[]`, `string[]`, `bool[]`, `mixed[]`, `int[][]` → `array<int>`, `array<string>`, `array<bool>`, `array<mixed>`, `array<array<int>>`).
- **Opérateur `==`** (et `<`/`>` découverts au passage — voir plus bas) : corrigés dans `examples/generics/test_syntax.oc` et `examples/tests/19_break_continueTest.oc` (`==` → `equal`). Commentaires d'en-tête obsolètes (`== != < <= > >=`) corrigés aussi dans `examples/04_conditions.oc`/`examples/15_operators.oc` (mentionnaient encore les symboles alors que le corps du fichier utilise déjà `equal`/`smaller`/`greater` depuis longtemps).
- **`<`/`>` obsolètes découverts en creusant** : au-delà de `==`, la même migration v0.2.0 a aussi remplacé `<`/`>` par `smaller`/`greater` (confirmé : `error: operator '<' has been removed — use 'smaller' instead`). Corrigés dans `examples/runtime/variables.oc` et `examples/tests/07_loopsTest.oc` (5 occurrences au total) — ces fichiers ne compilaient donc pas uniquement à cause de `T[]`.
- **Contradiction namespace imbriqué** : `docs/EBNF.md` §3.1 affirmait `namespace utils.http` "non supporté pour l'instant" — corrigé, avec renvoi vers l'exemple réel (`configs.routes` dans `examples/advanced/`).
- **20 fichiers `.bak` dans `examples/tests/`** : supprimés (artefacts de la migration `==`/`<`/`>`, confirmés obsolètes par diff contre leur fichier actuel avant suppression).

Vérifié : les 4 fichiers de test précédemment cassés compilent et passent tous leurs tests via `ocaraunit` (`386 PASS 0 FAIL 0 ERREUR(S)`, contre 4 erreurs de compilation avant). `make regression` termine maintenant en succès complet (`exit 0`), seule l'erreur `mainTest.oc`/`Printable` (bug séparé, voir [langage-imports-modules](langage-imports-modules.md)) subsiste sans faire échouer le process global.

## Consciemment non traité

- **Combinaisons génériques non illustrées** (`extends`+`implements`+`modules` combinés sur un `generic`, `Option<T>`, `QualifiedType` en annotation de type) : pas d'ajout de nouveaux exemples. Le système de types des génériques a des lacunes connues et non résolues (voir [langage-generiques](langage-generiques.md), Haute priorité) — un `extends` sur un `generic` n'est par exemple jamais vérifié sémantiquement (confirmé en observant `test_syntax.oc extends BaseResult`, une classe inexistante, compiler sans erreur). Ajouter des exemples combinés risquerait de heurter immédiatement ces lacunes plutôt que de simplement illustrer une syntaxe — hors périmètre d'un correctif "syntaxe obsolète".

## Fichiers clés

`examples/generics/`, `examples/tests/`, `examples/runtime/variables.oc`, `examples/04_conditions.oc`, `examples/15_operators.oc`, `docs/EBNF.md` (§3 namespaces).
