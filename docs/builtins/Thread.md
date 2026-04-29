# Thread

Classe de la bibliothèque runtime `ocara.Thread` — création et gestion de threads OS natifs (POSIX `pthread` via la stdlib Rust).

Contrairement aux autres builtins, `Thread` est une **classe d'instance** : chaque thread est représenté par un objet créé avec `use Thread()`.

```ocara
import ocara.Thread
// ou
import ocara.*
```

---

## Création

### `use Thread() → Thread`

Alloue un nouveau thread (non encore lancé). L'ID du thread est assigné immédiatement.

```ocara
var t:Thread = use Thread()
```

---

## Méthodes d'instance

### `t.run(f: Function) → void`

Lance la closure `f` dans un thread OS séparé. La closure ne doit pas accepter de paramètres.

Les variables capturées par la closure sont partagées via le mécanisme **shared cell** (heap promotion) — les mutations sont visibles depuis les deux côtés.

> **Attention** : les accès concurrents non synchronisés à des variables partagées constituent un *data race*. Utiliser `ocara.Mutex` pour la synchronisation thread-safe. Voir [Mutex](Mutex.md).

```ocara
var t:Thread = use Thread()
t.run(nameless(): void {
    IO::writeln("hello depuis le thread")
})
t.join()
```

### `t.join() → void`

Bloque le thread courant jusqu'à ce que le thread `t` se termine. Sans effet si le thread n'a pas été lancé ou a déjà été joint.

```ocara
t.join()
```

### `t.detach() → void`

Détache le thread (`fire-and-forget`). Après `detach()`, `join()` n'a plus d'effet. Le thread continue à s'exécuter en arrière-plan jusqu'à sa fin naturelle.

```ocara
t.detach()
// t se termine indépendamment du programme principal
```

### `t.id() → int`

Retourne l'identifiant unique du thread (entier incrémental, `1` pour le premier thread créé).

```ocara
IO::writeln(t.id())   // 1, 2, 3, …
```

---

## Méthodes statiques

### `Thread::sleep(ms: int) → void`

Suspend le **thread courant** pendant `ms` millisecondes.

```ocara
Thread::sleep(500)   // pause 500 ms
```

### `Thread::current_id() → int`

Retourne l'ID du thread courant. Retourne `0` pour le thread principal.

```ocara
IO::writeln(Thread::current_id())   // 0 dans main, 1..n dans les threads créés
```

---

## Exemple complet

```ocara
import ocara.IO
import ocara.Thread

function main(): void {
    var t1:Thread = use Thread()
    var t2:Thread = use Thread()

    t1.run(nameless(): void {
        Thread::sleep(100)
        IO::writeln("t1 terminé, id=")
        IO::writeln(Thread::current_id())
    })

    t2.run(nameless(): void {
        IO::writeln("t2 terminé, id=")
        IO::writeln(Thread::current_id())
    })

    t1.join()
    t2.join()
    IO::writeln("tous les threads terminés")
}
```

---

## Tableau récapitulatif

| Méthode | Type | Signature | Description |
|---|---|---|---|
| `run` | instance | `(f: Function) → void` | Lance la closure dans un thread OS |
| `join` | instance | `() → void` | Attend la fin du thread |
| `detach` | instance | `() → void` | Détache le thread (fire-and-forget) |
| `id` | instance | `() → int` | ID unique du thread |
| `Thread::sleep` | statique | `(ms: int) → void` | Pause en millisecondes |
| `Thread::current_id` | statique | `() → int` | ID du thread courant (0 = main) |

---

## Notes

- Les IDs sont assignés de manière atomique et croissante à partir de `1`. Le thread principal a l'ID `0`.
- Si `run()` n'est pas appelé, `join()` et `detach()` sont sans effet.
- Appeler `run()` deux fois sur le même objet `Thread` est un comportement non défini (le handle précédent est écrasé).
- La durée passée à `Thread::sleep()` est un minimum — le système peut dormir plus longtemps.

---

## Exceptions

### ThreadException

Deux méthodes peuvent lever une **ThreadException** :

**Codes d'erreur** :

| Code | Signification |
|------|---------------|
| 101  | Thread spawn failed (ressources insuffisantes) |
| 102  | Thread panicked during execution |

**Méthodes concernées** :

- `t.run(f)` — lève une exception si la création du thread OS échoue (code 101)
- `t.join()` — lève une exception si le thread a paniqué pendant son exécution (code 102)

**Exemples d'utilisation** :

```ocara
import ocara.IO
import ocara.Thread
import ocara.ThreadException

// Exemple 1 : Capture d'exception lors de la création
function main(): void {
    var t:Thread = use Thread()
    try {
        t.run(nameless(): void {
            IO::writeln("Thread started")
        })
        t.join()
    } on e is ThreadException {
        IO::writeln(`Erreur: ${e.message}`)
        IO::writeln(`Code: ${e.code}`)
    }
}

// Exemple 2 : Thread qui pourrait paniquer
function test_panic(): void {
    var t:Thread = use Thread()
    try {
        t.run(nameless(): void {
            // Code qui pourrait causer un panic
            IO::writeln("Running...")
        })
        t.join()  // Lève ThreadException si le thread a paniqué
    } on e is ThreadException {
        if e.code == 102 {
            IO::writeln("Le thread a paniqué")
        }
    }
}

// Exemple 3 : Gestion multiple avec d'autres exceptions
function complex_case(): void {
    var t:Thread = use Thread()
    try {
        t.run(nameless(): void {
            var result:int = Convert::str_to_int("invalid")
        })
        t.join()
    } on e is ThreadException {
        IO::writeln(`Thread error: ${e.message}`)
    } on e {
        IO::writeln(`Other error: ${e.message}`)
    }
}
```

**Notes** :

- Les méthodes **safe** (qui ne lèvent jamais d'exception) :
  - `t.id()` — toujours valide
  - `t.detach()` — toujours valide
  - `Thread::sleep(ms)` — toujours valide
  - `Thread::current_id()` — toujours valide
- Si `join()` est appelé sur un thread qui n'a pas été lancé ou déjà joint, il ne fait rien (pas d'exception)
- Les exceptions lancées à l'intérieur de la closure du thread sont capturées par `join()` et transformées en ThreadException code 102

---

## Voir aussi

- [Mutex](Mutex.md) — synchronisation thread-safe pour protéger les données partagées
- [docs/builtins/](../builtins/) — toutes les classes builtins
