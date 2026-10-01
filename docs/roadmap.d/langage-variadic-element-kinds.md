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

## À trancher

`sum_int(nums)` avec `nums:variadic<int>` : transmission du variadic
(« spread », comme `...nums`) ou erreur de type (un `array<int>` n'est pas un
`int`) ? Aujourd'hui accepté par la sema, faux à l'exécution.

## Priorité / Complexité

**Haute** (valeurs silencieusement fausses ou échec de codegen) —
**Légère** à **Structurel** selon la décision sur la transmission.

## Fichiers clés

`src/lower/stmt.d/statements.d/loops.rs` (variable de boucle), packing
variadic dans `src/lower/expr.d/lower.rs`, `src/lower/builder.d/program.rs`
(`fn_variadic_info`), `src/sema/typecheck.rs` (type d'un argument variadic).
