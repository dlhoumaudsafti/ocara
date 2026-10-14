# `for k has v in map` : valeur lue en `int` — corrigé

Trouvé en vérifiant `ocaracs --fix` : dans `for pays => capitale in
capitales` (`map<string, string>`), `${capitale}` et
`IO::writeln(capitale)` affichaient une adresse (`France → 4464680`). Même
chose pour les `string`/`bool`/`float` d'une `map<string, mixed>`
(`examples/09_maps.oc`, `actif = 384281506`).

Cause : `lower_for_map` (`src/lower/stmt.d/statements.d/loops.rs`) déclarait
la variable valeur `IrType::I64` en dur, quel que soit le type de valeur de
la map.

Correctif : la valeur est lue au type de valeur déclaré (`elem_types` de la
map), comme l'élément de `for x in array<T>`, et `Ptr` (`mixed`) si ce type
est inconnu. Test : `examples/tests/75_map_loop_value_typeTest.oc`.

## Reste à faire — audit du 2026-10-13

La valeur `v` est corrigée (`examples/tests/75_map_loop_value_typeTest.oc`). La syntaxe `=>` citée plus haut est obsolète (`for k has v`). Restent :

1. **Clé d'une `map<int,…>` lue comme chaîne.** `lower_for_map` déclare toujours la clé en `Ptr` et la lie comme `Type::String` (`src/lower/stmt.d/statements.d/loops.rs`, ~l.284-286) ; le runtime stocke toutes les clés en chaîne (`Map_keys` passe par `alloc_str`, `runtime/src/lib.rs` ~l.1265). La sema type pourtant `k` en `int` : `k * 10` donne une adresse (`6095146640`), `k + 1` donne `"11"`, `k equal 1` est toujours faux ; `mi.keys()` relu en `int` donne une adresse. Contraire à l'EBNF (~l.1096, « les clés peuvent être de n'importe quel type »). Correction : convertir la clé selon `K` (ou conserver le type des clés à l'exécution). **Haute — Structurel.**
2. `v.len()` quand la valeur est un `array<int>` rend `1` : voir [langage-methode-recepteur-indexe](langage-methode-recepteur-indexe.md).

**Point 1 corrigé le 2026-10-13** : une clé non-chaîne est boxée à l'écriture (littéral, `m[k]`, affectation indexée) et convertie en texte par le runtime (`key_to_string`) ; à la lecture, la clé de `for k has v` est reconvertie au type `K` (`__map_key_as`) et `keys()`/`Map::keys` rend un `array<K>` brut (argument caché du type de clé). `map<float, V>` plantait (SIGSEGV) : corrigé aussi. Test : `examples/tests/91_…` (`intKeysTest`).
