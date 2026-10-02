# `0` dans un tableau imbriqué encodé en JSON : valeur mémoire brute

## Constat (reproduit)

```ocara
var nested:array<array<int>> = [[1, 2], [0, 3]]
IO::writeln(JSON::encode(nested))   // [[1,2],[837928771,3]] — attendu [[1,2],[0,3]]
```

Le `0` du tableau INTERNE ressort comme un grand entier variable d'une
exécution à l'autre : une valeur lue en mémoire, pas un simple mauvais
formatage. Piste : `0` doit être boxé dans un conteneur `mixed` pour se
distinguer de `null` (voir `box_int_if_needed`, `runtime/src/tests/boxing.rs`)
— le littéral imbriqué ne passe probablement pas par ce boxing, et
l'encodeur JSON interprète alors la valeur comme un pointeur.

Échec de `examples/tests/40_json_yaml_concrete_containersTest.oc`
(`nestedConcreteArrayTest`) **masqué jusqu'ici** par ocaraunit (un échec
après une assertion réussie n'était jamais signalé — corrigé, voir
docs/roadmap.d/stdlib-convert-instance-methods.md).

## Correction

Cause réelle (pas l'encodeur JSON : `nested[1][0]` valait déjà l'adresse) :
un littéral `array`/`map` IMBRIQUÉ était toujours construit en `mixed`
(scalaires boxés), alors que la variable `array<array<int>>` le relisait
brut. `lower_array_literal`/`lower_map_literal` (`src/lower/expr.d/lower.rs`)
prennent maintenant le TYPE d'élément de destination (au lieu de l'ancien
`LiteralElemKind`, retiré) et le propagent récursivement. Le type de
destination est fourni pour une déclaration `var`/`const`, un `return`
(nouveau `LowerBuilder::ret_ast_ty`) et une affectation (variable, champ,
élément indexé — `declared_container_type`, `assignments.rs`).

Stocker ces scalaires bruts a exposé chaque chemin qui les supposait boxés,
corrigés dans la foulée :
- inférence de type d'une indexation chaînée (`a[1][1]`, `make()[1][1]`,
  `self.grid[i][j]`) — `elem_type_after_index`, avec une nouvelle table des
  types de retour AST (`IrModule::call_ret_types`) ;
- variable de boucle sur un `array<array<T>>` (type d'élément interne) ;
- destructeur/clone de classe (`__free_<C>`/`__clone_<C>`) : même choix de
  variante `shallow`/`concrete` que pour une variable possédée
  (`value_ownership_symbol`, `ownership.rs`) au lieu de `__value_free`
  générique, qui suivait un entier brut comme un pointeur (SEGFAULT, et
  `free(): invalid pointer` dans `examples/project`).

Tests : `examples/tests/70_typed_container_literalsTest.oc`,
`40_json_yaml_concrete_containersTest`/`41_nested_container_ownershipTest`.

Limite restante (représentation, pas une régression) : un tableau concret
placé dans un conteneur NON typé (`JSON::encode([flat, flat])`, argument
`mixed`) y garde ses scalaires bruts — un `0` y est lu comme `null`.

## Priorité / Complexité

**Haute, en tête** (lecture mémoire non initialisée/mal interprétée —
potentiellement un crash selon la valeur lue) — **Dangereuse** (boxing des
conteneurs imbriqués).

## Fichiers clés

`src/lower/expr.d/` (littéraux de tableau, `LiteralElemKind`),
`runtime/src/lib.rs` (encodage JSON, boxing), `examples/tests/40_json_yaml_concrete_containersTest.oc`.
