# Division entière par zéro : arrêt du programme sans exception

Statut : **à corriger** — constaté le 2026-10-18 en vérifiant l'état des types
numériques (voir [langage-types-numeriques-largeur](langage-types-numeriques-largeur.md)).

## Constat

`/` et `%` entre `int` sont émis en `sdiv`/`srem` Cranelift sans garde
(`src/codegen/emit.d/instructions.d/arithmetic.rs`) ; le processeur
interrompt le programme, sans exception Ocara rattrapable par `try`/`on` :

| Expression | Résultat |
|---|---|
| `x / 0` | arrêt, SIGILL (code 132) |
| `x % 0` | arrêt, SIGFPE (code 136) |
| `INT_MIN / -1` | arrêt, SIGFPE (code 136) |

```ocara
architecture permissive
import ocara.IO
function main(): int {
    var a:int = 7
    var z:int = 0
    IO::writeln(a / z)      // arrêt du processus
    IO::writeln("après")    // jamais atteint
    return 0
}
```

Une constante de classe calculée à la compilation rejette déjà ces cas
(E55, `docs/diagnostics.md`) ; l'exécution, elle, n'a aucune garde. La
division flottante n'est pas concernée (`1.0 / 0.0` donne `inf`).

## Correction attendue

Garde avant `sdiv`/`srem` (diviseur nul, et `INT_MIN / -1`) qui lève une
exception Ocara (`MathException` ?) ; documenter le comportement dans l'EBNF
(opérateurs arithmétiques).

## Validation

La reproduction lève une exception rattrapable ; un `try`/`on` autour de la
division poursuit l'exécution.

## Priorité / Complexité

**Haute** (arrêt brutal du programme) — **Légère**.
