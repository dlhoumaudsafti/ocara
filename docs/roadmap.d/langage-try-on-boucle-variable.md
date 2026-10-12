# Variable modifiée dans un `on` à l'intérieur d'une boucle : résultat faux et fuite

Statut : **à corriger** — trouvé le 2026-10-12 en testant le bilan des
fuites (`ocara --leak-check`).

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

## Pistes

- `try`/`on` repose sur `setjmp`/`longjmp` (runtime `__ocara_try_exec`) : une
  variable modifiée entre le `setjmp` et le `longjmp`, ou dans le bloc `on`,
  peut être restaurée à sa valeur du moment du `setjmp` si elle vit dans un
  registre. Ici `n` est capturée dans une cellule (le corps du `try` est
  une closure) : la cellule est-elle recréée à chaque tour au lieu d'être
  partagée, la mise à jour se faisant sur une copie ?
- La cellule non libérée par tour va dans le même sens (cellule créée par
  tour, jamais rendue).

## Vérification

La reproduction affiche 1, 2, 3, 4 ; `--leak-check` sur 1 000 tours :
aucune fuite.

## Priorité / Complexité

**Haute** (résultat faux et fuite) — lowering de `try`/`on`
(`src/lower/stmt.d/statements.d/exceptions.rs`), captures.
