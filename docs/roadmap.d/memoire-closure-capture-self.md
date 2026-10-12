# Closure capturant `self` retournée par une méthode : corruption du tas et fuite

Statut : **corrigé le 2026-10-12** — trouvé le même jour par le bilan des
fuites (`ocara --leak-check`, encart Benchmark de l'extension).

## Reproduction

```ocara
architecture permissive

class C {
    public property v:int
    init(v:int) { self.v = v }
    public method inc(step:int): Function<void()> {
        return nameless(): void {
            self.v = self.v + step
        }
    }
}

function once(): void {
    var c:C = use C(0)
    var f:Function<void()> = c.inc(1)
}

function main(): int {
    for i in 0 until 20000 {
        once()
    }
    return 0
}
```

- **Crash** : `malloc(): unaligned fastbin chunk detected 2` puis SIGABRT
  (corruption du tas), avec `var` comme avec `scoped c`, que la closure soit
  appelée ou non — dès qu'une méthode retourne une closure qui capture
  `self`.
- **Fuite** : avec 1 000 itérations (avant le crash), `--leak-check` rapporte
  `1000 × C (allouées 1000, libérées 0)` et `1000 × variable capturée
  (allouées 2000, libérées 1000)` : la cellule qui capture `self` n'est
  jamais rendue, et l'objet qu'elle retient non plus. Visible aussi sur
  `examples/25_nameless.oc` (2 `Counter`, 3 cellules).

## Cause et correction

Une méthode reçoit `self` en premier paramètre (type `mixed`, donc compté) :
`begin_function` le déclarait deux fois — locale simple, puis paramètre
compté retenu à l'entrée. Capturé par une closure, `self` est promu dans une
cellule :

- les **deux** déclarations relâchaient la cellule en fin de méthode →
  cellule libérée alors que l'environnement de la closure la tenait encore
  (utilisation après libération, corruption du tas sous charge) ;
- `promote_to_cell` retenait `self` une fois de plus pour la cellule, et la
  retenue d'entrée du paramètre n'était jamais rendue → objet jamais libéré.

Correction (`src/lower/stmt.d/rc.rs`) : la référence du paramètre compté
passe à la cellule (plus de retenue supplémentaire) ; la cellule n'est
relâchée que par l'entrée comptée. Test : `examples/tests/85_closure_capture_selfTest.oc`.

## Vérification

`ocara --leak-check` sur la reproduction : « aucune fuite », et exécution
à 200 000 itérations sans crash ; `examples/25_nameless.oc` propre.

## Priorité / Complexité

**Haute** (crash et fuite) — lowering des closures (`src/lower/expr.d/nameless.rs`,
`captures.rs`), runtime `rc.rs` (environnements et cellules).
