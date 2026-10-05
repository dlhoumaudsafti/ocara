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
