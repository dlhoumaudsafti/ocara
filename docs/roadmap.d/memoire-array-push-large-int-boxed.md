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

## Piste

Au site d'appel d'une méthode `Array`/`Map` qui écrit un élément, ne pas
boxer quand le type d'élément déclaré du receveur est concret (`elem_types`
connu, non `mixed`).

## Priorité / Complexité

**Haute** (valeur silencieusement fausse ; libération sûre grâce aux
variantes `shallow`/`concrete`, mais la cellule boxée fuit) — **Légère**.

## Fichiers clés

`src/lower/expr.d/lower.rs` (`box_arg_for_mixed_param`, appels builtin),
`src/lower/expr.d/helpers.rs`.
