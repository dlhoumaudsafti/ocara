# Fermeture créée dans une branche non exécutée : SIGSEGV — corrigé

Audit du 2026-10-19, corrigé le même jour.

## Constat

```ocara
function show(flag:bool): void {
    var x:int = 5
    if flag {
        var f:Function<int()> = nameless(): int { return x }
        f()
    }
    IO::writeln(Convert::intToStr(x))      // show(false) → SIGSEGV
}
```

Même chose dans une boucle (`for k … { if k equal 1 { out.push(nameless…k) } }`
plantait dès l'itération 0) et dans un `switch`.

## Cause

La capture promouvait la variable au tas à l'endroit de la fermeture, donc
seulement dans la branche prise ; la redirection `locals[name]` vers la
cellule restait en vigueur après le branchement (`src/lower/stmt.d/block.rs`).
Sur les autres chemins, la variable était encore un slot de pile, lu comme une
cellule (`__locked_cell_get`).

## Correctif

La pré-promotion des boucles, renommée `hoist_closure_promotions`
(`src/lower/expr.d/lower.rs`), s'applique aussi avant chaque branchement :
branches de `if`/`elseif`/`else` (conditions d'`elseif` comprises) et cas de
`switch` (`src/lower/stmt.d/statements.d/control_flow.rs`), bras de `match`
(`hoist_closure_promotions_in_exprs`). La cellule est créée sur tous les
chemins ; une variable déclarée dans un corps de boucle reste neuve à chaque
itération.

## Tests

`examples/tests/96_closure_branch_captureTest.oc` (branche non prise, `if`/
`elseif`/`else`, `switch`, `match`, branche dans une boucle, `switch` dans une
boucle, générateur) ; `--leak-check` sans fuite.
