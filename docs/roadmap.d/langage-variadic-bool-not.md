# `not f` faux sur un élément de `variadic<bool>` parcouru par `for`

## Constat (reproduit)

```ocara
function all_true(flags:variadic<bool>): bool {
    for f in flags {
        if not f { return false }
    }
    return true
}
all_true(true)         // false — attendu true
all_true(true, true)   // false
```

`flags[0]` retourne pourtant la bonne valeur. Piste : un `bool` stocké dans
le tableau variadic est BOXÉ (représentation `mixed`), et la variable de
boucle `f` est traitée comme un `bool` brut — `not` s'applique alors au
pointeur de la cellule boxée (toujours « vrai »), pas à la valeur.

Échec de `examples/tests/30_variadicTest.oc` (`variadicBoolTest`)
**masqué jusqu'ici** par ocaraunit.

## Priorité / Complexité

**Haute** (résultat silencieusement faux) — **Légère** (déballage de la
variable de boucle selon le type élément déclaré).

## Fichiers clés

`src/lower/stmt.d/` (lowering de `for x in ...`), boxing `bool`
(`runtime/src/lib.rs`), `examples/tests/30_variadicTest.oc`.
