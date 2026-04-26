# `ocara.Array` — Classe builtin

> Classe de manipulation de tableaux.  
> Toutes les méthodes sont **statiques** : elles s'appellent via `Array::<méthode>(args)`.  
> Fonctionne avec n'importe quel type d'élément (`int[]`, `string[]`, etc.).

---

## Import

```ocara
import ocara.Array        // importe uniquement Array
import ocara.*            // importe toutes les classes builtins
```

---

## Référence des méthodes

### `Array::len(arr)` → `int`

Retourne le nombre d'éléments du tableau.

```ocara
var n:int[] = [1, 2, 3]
Array::len(n)   // → 3
Array::len([])  // → 0
```

---

### `Array::push(arr, val)` → `void`

Ajoute `val` à la **fin** du tableau. Modifie le tableau en place.

```ocara
var t:int[] = [1, 2]
Array::push(t, 3)
// t est maintenant [1, 2, 3]
```

---

### `Array::pop(arr)` → `mixed`

Retire et retourne le **dernier** élément du tableau. Modifie le tableau en place.

```ocara
var t:int[] = [1, 2, 3]
scoped v:int = Array::pop(t)   // → 3
// t est maintenant [1, 2]
```

---

### `Array::first(arr)` → `mixed`

Retourne le premier élément sans modifier le tableau.

```ocara
Array::first([10, 20, 30])   // → 10
```

---

### `Array::last(arr)` → `mixed`

Retourne le dernier élément sans modifier le tableau.

```ocara
Array::last([10, 20, 30])   // → 30
```

---

### `Array::contains(arr, val)` → `bool`

Retourne `true` si `val` est présent dans le tableau (comparaison stricte).

```ocara
var t:int[] = [1, 2, 3]
Array::contains(t, 2)    // → true
Array::contains(t, 99)   // → false
```

---

### `Array::index_of(arr, val)` → `int`

Retourne l'index (0-basé) de la première occurrence de `val`, ou `-1` si absent.

```ocara
var t:string[] = ["a", "b", "c"]
Array::index_of(t, "b")    // → 1
Array::index_of(t, "z")    // → -1
```

---

### `Array::reverse(arr)` → `mixed[]`

Retourne un **nouvel** array contenant les éléments dans l'ordre inverse.  
Le tableau original n'est pas modifié.

```ocara
scoped inv:int[] = Array::reverse([1, 2, 3])
// → [3, 2, 1]
```

---

### `Array::slice(arr, from, to)` → `mixed[]`

Retourne un **nouvel** array contenant les éléments de l'index `from` (inclus) à `to` (exclu).  
Indices 0-basés.

| Paramètre | Type  | Description               |
|-----------|-------|---------------------------|
| `arr`     | `T[]` | Tableau source            |
| `from`    | `int` | Index de début (inclus)   |
| `to`      | `int` | Index de fin (exclu)      |

```ocara
var t:int[] = [10, 20, 30, 40, 50]
Array::slice(t, 1, 4)   // → [20, 30, 40]
Array::slice(t, 0, 2)   // → [10, 20]
```

Extraire les N derniers éléments :
```ocara
scoped tail:int[] = Array::slice(t, Array::len(t) - 2, Array::len(t))
// → [40, 50]
```

---

### `Array::join(arr, sep)` → `string`

Concatène tous les éléments du tableau en une chaîne, séparés par `sep`.

```ocara
Array::join(["a", "b", "c"], ", ")     // → "a, b, c"
Array::join([1, 2, 3], " | ")          // → "1 | 2 | 3"
Array::join(["seul"], "-")             // → "seul"
```

---

### `Array::sort(arr)` → `mixed[]`

Retourne un **nouvel** array trié en ordre naturel (numérique pour les `int[]`, lexicographique pour les `string[]`).  
Le tableau original n'est pas modifié.

```ocara
scoped t:int[]    = Array::sort([30, 10, 50, 20])
// → [10, 20, 30, 50]

scoped s:string[] = Array::sort(["banane", "pomme", "abricot"])
// → ["abricot", "banane", "pomme"]
```

---

## Combinaisons courantes

```ocara
import ocara.Array

function main(): int {

    // Dédupliquer un tableau
    var src:int[]    = [1, 2, 3, 2, 4, 1, 5]
    var unique:int[] = []
    for v in src {
        if Array::index_of(unique, v) == -1 {
            Array::push(unique, v)
        }
    }
    write(`unique : ${Array::join(unique, ", ")}`)   // 1, 2, 3, 4, 5

    // Extraire les 3 derniers éléments
    var data:int[]    = [10, 20, 30, 40, 50, 60]
    scoped tail:int[] = Array::slice(data, Array::len(data) - 3, Array::len(data))
    write(Array::join(tail, ", "))   // 40, 50, 60

    // Trier puis joindre
    var tags:string[]      = ["rust", "ocara", "cranelift", "llvm"]
    scoped sorted:string[] = Array::sort(tags)
    write(Array::join(sorted, " | "))   // cranelift | llvm | ocara | rust

    // Vérification avant accès
    var vide:int[] = []
    if Array::len(vide) == 0 {
        write("tableau vide")
    }

    return 0
}
```

---

## Conventions runtime

| Méthode Ocara        | Symbole runtime C  | Params Cranelift          | Retour  |
|----------------------|--------------------|---------------------------|---------|
| `Array::len`         | `Array_len`        | `I64`                     | `I64`   |
| `Array::push`        | `Array_push`       | `I64, I64`                | —       |
| `Array::pop`         | `Array_pop`        | `I64`                     | `I64`   |
| `Array::first`       | `Array_first`      | `I64`                     | `I64`   |
| `Array::last`        | `Array_last`       | `I64`                     | `I64`   |
| `Array::contains`    | `Array_contains`   | `I64, I64`                | `I64`   |
| `Array::index_of`    | `Array_index_of`   | `I64, I64`                | `I64`   |
| `Array::reverse`     | `Array_reverse`    | `I64`                     | `I64`   |
| `Array::slice`       | `Array_slice`      | `I64, I64, I64`           | `I64`   |
| `Array::join`        | `Array_join`       | `I64, I64`                | `I64`   |
| `Array::sort`        | `Array_sort`       | `I64`                     | `I64`   |

Les tableaux sont passés comme pointeurs `I64` vers la structure de tableau gérée par le runtime.  
`Array::contains` retourne `0` (false) ou `1` (true) encodé en `I64`.

---

## Voir aussi

- [examples/builtins/array.oc](../../examples/builtins/array.oc) — exemple complet exécutable
- [docs/EBNF.md](../EBNF.md) — grammaire formelle
