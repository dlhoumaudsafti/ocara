# Mutex

Classe builtin `ocara.Mutex` — synchronisation thread-safe pour protéger les accès concurrents aux données partagées.

Un **mutex** (mutual exclusion) garantit qu'un seul thread à la fois peut accéder à une section critique protégée. Tout autre thread tentant d'acquérir le verrou sera bloqué jusqu'à ce que le mutex soit déverrouillé.

`Mutex` est une **classe d'instance** : chaque mutex est représenté par un objet créé avec `use Mutex()`.

```ocara
import ocara.Mutex
// ou
import ocara.*
```

---

## Création

### `use Mutex() → Mutex`

Alloue un nouveau mutex (déverrouillé par défaut).

```ocara
var m:Mutex = use Mutex()
```

---

## Méthodes d'instance

### `m.lock() → void`

Verrouille le mutex. Si le mutex est déjà verrouillé par un autre thread, le thread courant est bloqué jusqu'à ce que le mutex soit disponible.

**Attention** : si le même thread tente de verrouiller deux fois le même mutex, cela provoque un **deadlock** (blocage permanent).

```ocara
var m:Mutex = use Mutex()
m.lock()
// section critique — un seul thread à la fois
m.unlock()
```

### `m.unlock() → void`

Déverrouille le mutex, permettant à d'autres threads d'acquérir le verrou.

**Règles importantes** :
- `unlock()` doit être appelé par le **même thread** qui a appelé `lock()`.
- Appeler `unlock()` sans `lock()` préalable est un comportement non défini.
- Toujours veiller à appeler `unlock()` après `lock()`, même en cas d'erreur.

```ocara
m.lock()
// opérations protégées
m.unlock()
```

### `m.try_lock() → bool`

Tente de verrouiller le mutex **sans bloquer**. Retourne `true` si le verrou a été acquis, `false` si le mutex était déjà verrouillé.

Si `try_lock()` retourne `true`, un appel à `unlock()` est requis plus tard.

```ocara
if m.try_lock() {
    // verrou acquis
    // section critique
    m.unlock()
} else {
    // mutex déjà verrouillé, faire autre chose
}
```

---

## Exemple avec Thread

Protection d'une variable partagée entre threads :

```ocara
import ocara.IO
import ocara.Thread
import ocara.Mutex

function main(): void {
    var counter:int = 0
    var m:Mutex = use Mutex()
    
    var t1:Thread = use Thread()
    var t2:Thread = use Thread()
    
    t1.run(nameless(): void {
        var i:int = 0
        while i < 1000 {
            m.lock()
            counter = counter + 1
            m.unlock()
            i = i + 1
        }
    })
    
    t2.run(nameless(): void {
        var i:int = 0
        while i < 1000 {
            m.lock()
            counter = counter + 1
            m.unlock()
            i = i + 1
        }
    })
    
    t1.join()
    t2.join()
    
    IO::write("counter final = ")
    IO::writeln(counter)  // 2000 (garanti thread-safe)
}
```

---

## Exemple avec try_lock

```ocara
import ocara.IO
import ocara.Mutex

function main(): void {
    var m:Mutex = use Mutex()
    
    if m.try_lock() {
        IO::writeln("verrou acquis")
        // section critique
        m.unlock()
    } else {
        IO::writeln("mutex déjà verrouillé")
    }
}
```

---

## Tableau récapitulatif

| Méthode | Signature | Description |
|---|---|---|
| `lock` | `() → void` | Verrouille le mutex (bloquant) |
| `unlock` | `() → void` | Déverrouille le mutex |
| `try_lock` | `() → bool` | Tente de verrouiller sans bloquer |

---

## Bonnes pratiques

1. **Toujours unlock** : chaque `lock()` doit être suivi d'un `unlock()`.
2. **Sections critiques courtes** : minimiser le temps passé entre `lock()` et `unlock()`.
3. **Éviter les deadlocks** : ne jamais verrouiller deux fois le même mutex depuis le même thread.
4. **Un mutex par ressource** : utiliser un mutex distinct pour chaque donnée partagée indépendante.
5. **try_lock pour éviter les blocages** : préférer `try_lock()` quand un échec est acceptable.

---

## Notes

- Le mutex utilise l'implémentation système native (pthread sur Linux/macOS, SRWLOCK sur Windows via Rust stdlib).
- Un mutex non déverrouillé avant la fin du programme ne provoque pas de fuite mémoire, mais peut causer des blocages si d'autres threads attendent.
- Pour des patterns plus avancés (read-write locks, condition variables), des classes futures seront ajoutées.

---

## Voir aussi

- [Thread](Thread.md) — création et gestion de threads
- [docs/builtins/](../builtins/) — toutes les classes builtins
