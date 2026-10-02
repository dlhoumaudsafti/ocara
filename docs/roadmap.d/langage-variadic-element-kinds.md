# Variadics : éléments `array`/`Function`, transmission d'un variadic, méthode d'instance

## Constats (reproduits — préexistants, découverts en corrigeant langage-variadic-bool-not)

Dans `examples/30_variadic.oc` (les sorties fausses n'étaient vérifiées par
aucun test) :

```ocara
function flatten_arrays(arrays:variadic<array<int>>): int {
    var count:int = 0
    for arr in arrays { count = count + Array::len(arr) }
    return count
}
flatten_arrays([1,2,3], [4,5], [6,7,8,9])   // grand nombre aberrant — attendu 9

function apply_functions(val:int, funcs:variadic<Function<int(int)>>): int {
    var result:int = val
    for f in funcs { result = f(result) }   // la variable de boucle n'est pas
    return result                           // reconnue comme fonction
}
apply_functions(5, double, triple)          // 0 — attendu 30

function default_sum(nums:variadic<int>): int {
    return sum_int(nums)                    // `nums` (un tableau) passé comme UN
}                                           // seul élément entier : valeur aberrante
```

Et, découvert en écrivant `examples/tests/70_typed_container_literalsTest.oc` :
un paramètre variadic sur une **méthode d'instance** n'est jamais empaqueté
(`fn_variadic_info` ne couvre que fonctions libres et méthodes statiques) —
`self.allTrue(true, true)` échoue au codegen (`mismatched argument count`).

## Décision et correction

**Transmission** (David) : `sum_int(nums)` avec `nums:variadic<int>` doit
simplement fonctionner (→ 30) — un variadic `T` passé SEUL à la place d'un
`variadic<T>` est transmis tel quel. Pour un élément pointeur
(`variadic<array<T>>`, `variadic<mixed>`), seulement si l'argument est
lui-même un paramètre variadic (un tableau quelconque y reste un élément).

Mise en œuvre (`src/lower/expr.d/lower.rs`, `pack_variadic_args`) :
- **Représentation unifiée** : le tableau variadic stocke ses éléments
  exactement comme un `array<T>` (brut pour `int`/`float`/`bool`, boxé pour
  `mixed`/union) — un `variadic<T>` EST un `array<T>` dans le corps, ce qui
  supprime le déballage spécial des boucles/indexations ajouté par
  [langage-variadic-bool-not](langage-variadic-bool-not.md) ;
- **un seul empaquetage** pour fonction libre, méthode statique et méthode
  d'INSTANCE (désormais déclarée dans `fn_variadic_info`) ;
- `lower_func` n'enregistrait plus le bon type d'élément pour
  `variadic<array<T>>`/`variadic<map<…>>` (écrasé par celui du tableau
  INTERNE — `Array::len(arr)` recevait un tableau boxé comme entier) ;
- variable de boucle sur `Function<...>` désormais appelable ; variable de
  boucle sur un tableau imbriqué reconnue comme `Array` (`row.len()`).

Les sorties de `examples/30_variadic.oc` sont désormais justes
(`flatten_arrays` = 9, `apply_functions` = 30, `default_sum(10, 20)` = 30).
Tests : `examples/tests/71_variadic_forwarding_and_pushTest.oc`. Documenté
dans `docs/EBNF.md` §14.2.

## Priorité / Complexité

**Haute** (valeurs silencieusement fausses ou échec de codegen) —
**Légère** à **Structurel** selon la décision sur la transmission.

## Fichiers clés

`src/lower/stmt.d/statements.d/loops.rs` (variable de boucle), packing
variadic dans `src/lower/expr.d/lower.rs`, `src/lower/builder.d/program.rs`
(`fn_variadic_info`), `src/sema/typecheck.rs` (type d'un argument variadic).
