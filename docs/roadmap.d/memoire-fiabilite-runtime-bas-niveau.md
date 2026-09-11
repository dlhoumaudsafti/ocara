# Fragilités bas niveau du runtime mémoire

Trois points latents, non déclenchés aujourd'hui en pratique mais sans aucune protection structurelle :

- **Recalcul de taille sans header fiable** : `__object_free(ptr, n_fields)` (runtime/src/lib.rs:2244-2252) recalcule la taille à partir de `n_fields` fourni par le compilateur (pas stocké dans un header mémoire) ; `free_str` (runtime/src/lib.rs:167-183) recalcule la longueur via le terminateur NUL. Une divergence entre le layout compilé et l'appel runtime, ou une string avec un octet NUL interne, ferait passer un `Layout` faux à `dealloc` → crash/abort glibc.
- **Tag d'exception confondu avec `map`** : le tag `0x03` utilisé pour les exceptions est identique à `TAG_MAP` (runtime/src/exception.rs:222 vs runtime/src/typecheck.rs:18). Non déclenché aujourd'hui car aucune exception n'est actuellement `scoped`, mais si ça devait arriver, `__value_free`/`__is_map` ferait un `drop_in_place` sur le mauvais type Rust.
- **Détection de type par heuristique sur la valeur d'un entier** : `read_tag` (runtime/src/typecheck.rs:38-44) déréférence `*(val-8)` dès qu'un entier est `>= 65536` et multiple de 4 — un entier Ocara ordinaire qui remplit ce critère et passé à `x is string` lit une adresse mémoire arbitraire.
- **Portabilité `Mutex`** : `PthreadMutex` est un tableau d'octets de taille hardcodée par plateforme (`[u8;40]` Linux, `[u8;64]` macOS/fallback, runtime/src/mutex.rs:31-37), jamais vérifié contre la vraie taille de `pthread_mutex_t` de l'ABI cible.

## Ampleur

Chaque point est un correctif localisé (tag dédié pour les exceptions, header de taille réelle pour les objets, remplacement du tableau d'octets par le type `libc::pthread_mutex_t`) — mais la détection heuristique de type par valeur d'entier est plus délicate à changer sans revoir la représentation des entiers/pointeurs elle-même.

## Fichiers clés

`runtime/src/lib.rs`, `runtime/src/exception.rs`, `runtime/src/typecheck.rs`, `runtime/src/mutex.rs`.
