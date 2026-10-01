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

## Priorité / Complexité

**Haute, en tête** (lecture mémoire non initialisée/mal interprétée —
potentiellement un crash selon la valeur lue) — **Dangereuse** (boxing des
conteneurs imbriqués).

## Fichiers clés

`src/lower/expr.d/` (littéraux de tableau, `LiteralElemKind`),
`runtime/src/lib.rs` (encodage JSON, boxing), `examples/tests/40_json_yaml_concrete_containersTest.oc`.
