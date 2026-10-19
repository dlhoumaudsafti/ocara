# Fermeture créée dans une branche non exécutée : SIGSEGV sur la variable capturée

Audit du 2026-10-19. Reproduit sur le binaire courant.

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

Même chose dans une boucle (`for k in 0 until 3 { if k equal 1 {
out.push(nameless(): int { return k }) } }` plante dès l'itération 0) et dans
un `switch`.

## Cause

`src/lower/stmt.d/block.rs` (restauration de fin de bloc) garde la
redirection `locals[name]` vers la cellule tas, alors que la cellule n'est
allouée que dans la branche prise : l'autre chemin lit le slot de pile comme
une cellule (`__locked_cell_get`).

## Piste

Promouvoir la variable AVANT le branchement (comme
`hoist_closure_promotions_before_loop` le fait pour les boucles) : un
pré-scan des `if`/`switch`/`match` qui contiennent une fermeture capturant une
variable externe.

## Priorité / Complexité

**Haute** (crash) — **Légère**.

## Fichiers clés

`src/lower/stmt.d/block.rs`, `src/lower/expr.d/lower.rs`
(`hoist_closure_promotions_before_loop`), `src/lower/stmt.d/statements.d/control_flow.rs`.
