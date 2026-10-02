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

## Correction

Le site d'appel boxe chaque argument variadic (`float`/`bool` toujours,
`int` s'il est ambigu avec un pointeur), mais la boucle lisait l'élément
brut — un commentaire l'assumait (« les float/bool nécessiteraient unboxing
mais pour l'instant on les laisse »). `lower_func` enregistre maintenant le
type d'élément d'un paramètre variadic, et `unbox_variadic_elem`
(`src/lower/expr.d/helpers.rs`) déballe l'élément à la lecture, dans une
boucle comme sur une indexation (`flags[0]`). Les variadics `int`/`float`/
`string` restent corrects (vérifié). Autres défauts variadics découverts au
passage, distincts : voir [langage-variadic-element-kinds](langage-variadic-element-kinds.md).

## Priorité / Complexité

**Haute** (résultat silencieusement faux) — **Légère**.

## Fichiers clés

`src/lower/stmt.d/` (lowering de `for x in ...`), boxing `bool`
(`runtime/src/lib.rs`), `examples/tests/30_variadicTest.oc`.
