# Grand entier ajouté par `Array::push`/`arr.push` dans un `array<int>` : relu comme une adresse

## Constat (reproduit, préexistant — vérifié sur le compilateur d'avant)

```ocara
var arr:array<int> = [1]
Array::push(arr, 7777777)
arr.push(6666666)
IO::writeln(`${arr[1]} ${arr[2]}`)   // deux grands nombres aberrants
```

Le second paramètre de `Array::push` est `mixed` : l'argument est boxé
(`__box_int_for_mixed`, au-delà de `PTR_THRESHOLD`), alors qu'un
`array<int>` stocke ses éléments BRUTS et les relit comme tels. Même
famille pour `Array::set`, `Map::set` sur un conteneur concret, et
`float`/`bool` (toujours boxés) — à vérifier un par un.

## Correction

`stores_raw_into_container` (`src/lower/expr.d/helpers.rs`) : l'argument
VALEUR de `Array::push`/`Array::set`/`Map::set` (formes statique et sucrée
`arr.push(v)`) est passé brut quand le conteneur a un type d'élément
concret (`int`/`float`/`bool`, receveur variable, champ ou élément indexé —
`elem_type_after_index`) ; un conteneur `mixed` continue de boxer. Tests :
`examples/tests/71_variadic_forwarding_and_pushTest.oc`.

## Priorité / Complexité

**Haute** (valeur silencieusement fausse ; libération sûre grâce aux
variantes `shallow`/`concrete`, mais la cellule boxée fuit) — **Légère**.

## Fichiers clés

`src/lower/expr.d/lower.rs` (`box_arg_for_mixed_param`, appels builtin),
`src/lower/expr.d/helpers.rs`.
