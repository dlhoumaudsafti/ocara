# Fragilités bas niveau du runtime mémoire

Quatre points latents identifiés initialement — deux corrigés, deux évalués et jugés hors de portée d'un correctif ponctuel.

## ✅ Tag d'exception confondu avec `TAG_MAP` — corrigé

Le tag `0x03` utilisé pour les exceptions (`alloc_exception`, `runtime/src/exception.rs`) était **littéralement identique** à `TAG_MAP` (`runtime/src/typecheck.rs`) — pas juste une collision théorique : confirmé par reproduction, une `ArrayException` capturée par un `on e { }` répondait `true` à `e is map<string, mixed>`.

```ocara
try {
    var x:int = arr.pop()   // arr vide → ArrayException
} on e {
    if e is map<string, mixed> {
        IO::writeln("MISCLASSIFIED as map!")   // s'affichait avant ce correctif
    }
}
```

**Corrigé** : nouveau tag dédié `TAG_EXCEPTION = 7` (`runtime/src/typecheck.rs`), utilisé par `alloc_exception` à la place de `0x03`. Vérifié : le cas ci-dessus répond maintenant correctement `false` ; `get_value_type` (utilisé par `JSON::encode`/comparaisons strictes) retombe correctement sur "primitif" pour ce nouveau tag, comme pour tout tag non reconnu — aucune exception n'étant aujourd'hui `scoped`/`consumed`, `__value_free`/`__value_clone` ne sont pas concernés.

## ✅ Portabilité `Mutex` — corrigée

`PthreadMutex` était un tableau d'octets de taille hardcodée par plateforme (`[u8;40]` Linux, `[u8;64]` macOS/fallback), jamais vérifiée contre la vraie taille de `pthread_mutex_t` de l'ABI cible — une libc dont la structure serait plus grande (ex. une musl ou une variante Linux différente de celle supposée) aurait laissé `pthread_mutex_init` écrire hors des bornes de l'allocation.

**Corrigé** : remplacement par `libc::pthread_mutex_t` (nouvelle dépendance `libc` dans `runtime/Cargo.toml`), dont la taille/l'alignement sont garantis corrects pour la cible de compilation réelle. Les 5 fonctions `pthread_mutex_*` appelées à la main sont aussi remplacées par celles de `libc` directement (au lieu de redéclarations manuelles risquant de diverger de la signature réelle). Vérifié : `make regression` (exemple `mutex.oc`, double-`.destroy()` E25) sans régression.

## Non traités — évalués, jugés plus larges qu'un correctif ponctuel

- **Recalcul de taille sans header fiable** (`__object_free`/`free_str`, `runtime/src/lib.rs`) : une vraie correction demanderait de stocker la taille réelle dans le header de CHAQUE allocation heap (string/array/map/objet/exception/fonction), pas seulement celles concernées par ce point — un changement de format de header impactant à la fois le runtime ET le codegen (`Inst::Alloc`, `src/lower/builder.d/class_ownership.rs` qui calcule `n_fields`), pas un correctif localisé.
- **Détection de type par heuristique sur la valeur d'un entier** (`read_tag`, `runtime/src/typecheck.rs`) : confirmé **bien plus grave** qu'anticipé — pas juste une mauvaise classification, un **SEGFAULT reproductible** sur du code parfaitement ordinaire :
  ```ocara
  var n:mixed = 1000000
  if n is string { ... }   // SEGFAULT : déréférence l'adresse (1000000 - 8)
  ```
  Toute valeur `mixed` dont la valeur est `>= 65536` et multiple de 4 (une contrainte que N'IMPORTE QUEL entier ordinaire peut remplir par hasard) fait lire une adresse mémoire arbitraire. Une vraie correction demande soit de revoir la représentation des entiers/pointeurs (tagged integers, un bit dédié), soit de vérifier qu'une page est réellement mappée avant de déréférencer (`mincore()` ou équivalent, coût d'un syscall par vérification, spécifique à la plateforme) — les deux dépassent largement un correctif ponctuel. Documenté ici comme une découverte confirmée mais non traitée : à prioriser dans une session dédiée compte tenu de la sévérité (SEGFAULT, pas juste une valeur fausse).

## Fichiers clés

`runtime/src/typecheck.rs` (`TAG_EXCEPTION`, `read_tag`), `runtime/src/exception.rs` (`alloc_exception`), `runtime/src/mutex.rs` (`libc::pthread_mutex_t`), `runtime/Cargo.toml` (dépendance `libc`), `runtime/src/lib.rs` (`__object_free`, `free_str`, non touchés).
