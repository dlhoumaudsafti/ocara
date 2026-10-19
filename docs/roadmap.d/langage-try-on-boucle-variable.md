# Variable modifiée dans un `on` à l'intérieur d'une boucle : résultat faux et fuite

Statut : **corrigé le 2026-10-12** — trouvé le même jour en testant le bilan
des fuites (`ocara --leak-check`).

## Reproduction

```ocara
architecture permissive
import ocara.IO

function fail(): void {
    raise "x"
}

function main(): int {
    var n:int = 0
    for i in 0 until 4 {
        try {
            fail()
        } on e {
            n = n + 1
        }
        IO::writeln(`i=${i} n=${n}`)
    }
    IO::writeln(`fin n=${n}`)   // attendu 4, obtenu 1
    return 0
}
```

Sortie : `n=1` à chaque tour, `fin n=1` — attendu 1, 2, 3, 4. Même résultat
avec `while`. Avec 1 000 tours, `--leak-check` rapporte aussi
`999 × variable capturée (allouées 2000, libérées 1001)` : une cellule
capturée fuit par tour.

## Cause et correction

Le corps d'un `try` et ses blocs `on` sont des fonctions séparées : une
variable qu'ils lisent ou écrivent est promue dans une cellule partagée. La
promotion était émise à l'endroit du `try` — dans le corps de la boucle,
donc exécutée à chaque tour : une nouvelle cellule, initialisée depuis
l'emplacement d'origine (resté à 0 puisque les écritures vont dans la
cellule), et la cellule du tour précédent jamais rendue.

Les closures avaient le même problème, réglé par une promotion anticipée
avant la boucle (`hoist_closure_promotions`) ; sa recherche des
captures ne comptait pas les références directes d'un `try`. Correction
(`src/lower/expr.d/captures.rs`) : dans un `try`, le corps et les blocs `on`
comptent leurs références directes — la variable est promue une fois, avant
la boucle, et chaque tour réutilise la même cellule. Test :
`examples/tests/86_try_on_loop_variableTest.oc`.

## Vérification

La reproduction affiche 1, 2, 3, 4 ; `--leak-check` sur 1 000 tours :
aucune fuite.

## Priorité / Complexité

**Haute** (résultat faux et fuite) — lowering de `try`/`on`
(`src/lower/stmt.d/statements.d/exceptions.rs`), captures.
