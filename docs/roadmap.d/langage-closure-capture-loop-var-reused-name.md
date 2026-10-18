# Fermeture capturant une variable de boucle dont le nom est réutilisé : valeurs fausses puis blocage

## Constat (2026-10-18)

Une fermeture créée dans une boucle capture la variable de boucle. Si ce nom
a déjà servi de variable à une boucle précédente de la même fonction :

- les valeurs capturées sont fausses : `222` au lieu de `012` avec une plage,
  une valeur arbitraire (`52686116312`) avec un tableau littéral ;
- le programme se bloque ensuite (pas de fin, `timeout` → rc 124).

Avec deux noms distincts (`i` puis `j`), le résultat est juste (`012`). Le
défaut est déjà présent avant le passage de `for … until` en compteur : il
ne vient pas de la boucle de plage.

## Reproduction

```ocara
architecture permissive
import ocara.IO
function main(): int {
    var s:string = ""
    for i in 0 until 3 { s = s + i.toStr() }
    IO::writeln(s)
    var fns:array<Function<int()>> = []
    for i in 0 until 3 {
        fns.push(nameless(): int { return i })
    }
    s = ""
    for f in fns { s = s + f().toStr() }
    IO::writeln(s)       // attendu 012, obtenu 222, puis blocage
    return 0
}
```

## Piste

La promotion sur le tas des variables capturées est indexée par nom
(`hoist_closure_promotions_before_loop`, `src/lower/expr.d/lower.rs`).
Le nom de la première boucle reste probablement associé à son ancienne
cellule (ou slot), si bien que la seconde boucle n'en crée pas une neuve
par itération, et que la cellule est partagée ou libérée deux fois.

## Priorité / Complexité

**Haute** (valeurs fausses silencieuses, puis blocage) — **Dangereuse**
(promotion des captures, comptage de références).

## Fichiers clés

`src/lower/expr.d/lower.rs` (`hoist_closure_promotions_before_loop`),
`src/lower/stmt.d/statements.d/loops.rs`, `src/lower/stmt.d/statements.d/`.
