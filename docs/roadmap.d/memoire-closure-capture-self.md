# Closure capturant `self` retournée par une méthode : corruption du tas et fuite

Statut : **à corriger** — trouvé le 2026-10-12 par le bilan des fuites
(`ocara --leak-check`, encart Benchmark de l'extension).

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

## Pistes

- Capture de `self` dans une méthode : la cellule de `self` est-elle
  retenue (compte +1) à la capture et rendue à la destruction de
  l'environnement de la closure (`Kind::Env`, `for_each_child`) ? L'écart
  allouées/libérées (2 cellules par appel, 1 libérée) désigne la cellule de
  `self`.
- Corruption : une libération en trop (cellule ou objet rendu deux fois, ou
  rendu par le scope de la méthode alors que la closure le garde) expliquerait
  l'erreur de `malloc` sous charge.

## Vérification

`ocara --leak-check` sur la reproduction : « aucune fuite », et exécution
à 200 000 itérations sans crash ; `examples/25_nameless.oc` propre.

## Priorité / Complexité

**Haute** (crash et fuite) — lowering des closures (`src/lower/expr.d/nameless.rs`,
`captures.rs`), runtime `rc.rs` (environnements et cellules).
