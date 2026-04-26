# `ocara.Math` — Classe builtin

> Classe de fonctions mathématiques.  
> Toutes les méthodes sont **statiques** : elles s'appellent via `Math::<méthode>(args)`.

---

## Import

```ocara
import ocara.Math        // importe uniquement Math
import ocara.*           // importe toutes les classes builtins
```

---

## Constantes de classe

| Constante   | Type    | Valeur                  | Description               |
|-------------|---------|-------------------------|---------------------------|
| `Math::PI`  | `float` | `3.14159265358979`      | π — rapport circonférence/diamètre |
| `Math::E`   | `float` | `2.71828182845904`      | e — base du logarithme naturel |
| `Math::TAU` | `float` | `6.28318530717958`      | τ = 2π                    |
| `Math::INF` | `float` | `+∞`                   | Infini positif            |

```ocara
import ocara.Math

write(Math::PI)    // 3.14159265358979
write(Math::TAU)   // 6.28318530717958
```

---

## Référence des méthodes

### `Math::abs(n)` → `int`

Retourne la valeur absolue de l'entier `n`.

```ocara
Math::abs(-42)   // → 42
Math::abs(7)     // → 7
Math::abs(0)     // → 0
```

---

### `Math::min(a, b)` → `int`

Retourne le plus petit des deux entiers.

```ocara
Math::min(10, 3)    // → 3
Math::min(-5, -1)   // → -5
```

---

### `Math::max(a, b)` → `int`

Retourne le plus grand des deux entiers.

```ocara
Math::max(10, 3)    // → 10
Math::max(-5, -1)   // → -1
```

---

### `Math::pow(base, exp)` → `int`

Calcule `base` élevé à la puissance `exp` (entiers positifs).

| Paramètre | Type  | Description    |
|-----------|-------|----------------|
| `base`    | `int` | Base           |
| `exp`     | `int` | Exposant ≥ 0   |

```ocara
Math::pow(2, 10)   // → 1024
Math::pow(3, 4)    // → 81
Math::pow(5, 0)    // → 1
```

---

### `Math::clamp(n, lo, hi)` → `int`

Borne la valeur `n` dans l'intervalle `[lo, hi]`.  
Équivalent à `Math::max(lo, Math::min(n, hi))`.

| Paramètre | Type  | Description       |
|-----------|-------|-------------------|
| `n`       | `int` | Valeur à borner   |
| `lo`      | `int` | Borne inférieure  |
| `hi`      | `int` | Borne supérieure  |

```ocara
Math::clamp(150, 0, 100)   // → 100  (au-dessus)
Math::clamp(-5,  0, 100)   // → 0    (en-dessous)
Math::clamp(42,  0, 100)   // → 42   (dans la plage)
```

---

### `Math::sqrt(n)` → `float`

Retourne la racine carrée de `n` (float).

```ocara
Math::sqrt(16.0)   // → 4.0
Math::sqrt(2.0)    // → 1.4142135623730951
Math::sqrt(0.0)    // → 0.0
```

---

### `Math::floor(n)` → `int`

Arrondit `n` à l'entier inférieur.

```ocara
Math::floor(3.9)    // → 3
Math::floor(-1.1)   // → -2
```

---

### `Math::ceil(n)` → `int`

Arrondit `n` à l'entier supérieur.

```ocara
Math::ceil(3.1)    // → 4
Math::ceil(-1.9)   // → -1
```

---

### `Math::round(n)` → `int`

Arrondit `n` à l'entier le plus proche (0.5 → supérieur).

```ocara
Math::round(3.4)   // → 3
Math::round(3.5)   // → 4
Math::round(-2.5)  // → -2
```

---

## Combinaisons courantes

```ocara
import ocara.Math

function main(): int {

    // Aire d'un cercle
    var r:float = 5.0
    scoped aire:float = Math::PI * r * r
    write(`Aire cercle r=5 : ${aire}`)     // ~78.53

    // Distance euclidienne (Pythagore)
    var dx:int = 3
    var dy:int = 4
    scoped dist:float = Math::sqrt(Math::pow(dx, 2) + Math::pow(dy, 2))
    write(`Distance = ${dist}`)            // 5.0

    // Score normalisé entre 0 et 100
    var score:int = 120
    scoped clamped:int = Math::clamp(score, 0, 100)
    write(`Score : ${clamped}`)            // 100

    // Arrondi d'une valeur calculée
    scoped pages:int = Math::ceil(157.0 / 10.0)
    write(`Pages nécessaires : ${pages}`)  // 16

    return 0
}
```

---

## Conventions runtime

Les méthodes sont implémentées côté runtime C sous le préfixe `Math_` :

| Méthode Ocara    | Symbole runtime C | Params Cranelift          | Retour Cranelift |
|------------------|-------------------|---------------------------|------------------|
| `Math::abs`      | `Math_abs`        | `I64`                     | `I64`            |
| `Math::min`      | `Math_min`        | `I64, I64`                | `I64`            |
| `Math::max`      | `Math_max`        | `I64, I64`                | `I64`            |
| `Math::pow`      | `Math_pow`        | `I64, I64`                | `I64`            |
| `Math::clamp`    | `Math_clamp`      | `I64, I64, I64`           | `I64`            |
| `Math::sqrt`     | `Math_sqrt`       | `F64`                     | `F64`            |
| `Math::floor`    | `Math_floor`      | `F64`                     | `I64`            |
| `Math::ceil`     | `Math_ceil`       | `F64`                     | `I64`            |
| `Math::round`    | `Math_round`      | `F64`                     | `I64`            |

---

## Voir aussi

- [examples/builtins/math.oc](../../examples/builtins/math.oc) — exemple complet exécutable
- [docs/EBNF.md](../EBNF.md) — grammaire formelle
