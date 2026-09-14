# Fragilités bas niveau du runtime mémoire

Quatre points latents identifiés initialement — trois corrigés, un évalué et jugé hors de portée d'un correctif ponctuel.

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

## ✅ Corrigé — `free_str` ne recalcule plus une taille potentiellement fausse

**Des deux fonctions initialement citées ici, une seule portait un risque réel** : `free_str` retrouvait la longueur d'une string en cherchant son premier octet NUL depuis le pointeur — sous-estimation garantie si la string contient un NUL **interne**, ce qui est un cas atteignable (`\0` est un échappement de chaîne Ocara valide, `"a\0b"` compile et alloue normalement). Une longueur sous-estimée passe un `Layout` trop court à `dealloc` : UB (l'API Rust `alloc`/`dealloc` exige que le `Layout` de libération corresponde exactement à celui de l'allocation), potentiellement une corruption du tas.

`__object_free` (le second cas cité), en réexamen, ne portait en réalité **aucun risque de divergence** : `n_fields` provient d'une seule et même source (`module.class_layouts[Classe].len()`, immuable après compilation), relue identiquement au moment de l'allocation (`Inst::Alloc`, `src/codegen/emit.d/instructions.d/memory.rs`) et au moment de la libération (`src/lower/builder.d/class_ownership.rs`) — les deux lectures ne peuvent pas diverger au sein d'un même programme compilé. Laissé tel quel : pas de changement nécessaire, le risque était mal caractérisé au moment où ce point a été noté.

**Corrigé** (`alloc_str`/`free_str`, `runtime/src/lib.rs`) — sans le changement de format de header global initialement envisagé (qui aurait touché `Inst::Alloc` et toutes les autres allocations heap, cf. ancienne version de cette section) : uniquement le layout d'une string possédée gagne une case de 8 octets, AVANT le tag existant (qui reste à l'offset habituel `val - 8`, donc invisible de `read_tag`/`ptr_to_str`/tout le reste du runtime) :

```
avant : [tag:8][données...][NUL]
après : [len:8][tag:8][données...][NUL]
```

`free_str` lit maintenant `len` directement au lieu de le recalculer. Vérifié : régression complète sans changement (le format ne change le comportement OBSERVABLE d'aucun appelant, seul `free_str` lit `len`) ; un test dédié (`examples/tests/34_string_nul_safetyTest.oc`) alloue/libère en boucle des `scoped string` contenant un NUL interne, entrelacées avec d'autres allocations de taille voisine, sans crash. **Non vérifié empiriquement** : que l'ancien code plantait réellement sur ce système (glibc `free()` ne vérifie pas nécessairement la taille annoncée) — `valgrind` (qui l'aurait détecté à coup sûr) n'est pas disponible dans cet environnement ; la correction reste justifiée par le contrat documenté de `std::alloc::{alloc, dealloc}`, indépendamment de la démonstration empirique.

**Volontairement non traité, hors périmètre** : `ptr_to_str` (et donc l'affichage, la comparaison, `String::*`, `JSON::encode`, ...) reste basé sur `CStr::from_ptr`, qui tronque toujours au premier NUL — une string à NUL interne reste donc **affichée/comparée tronquée** partout ailleurs dans le langage. Ce correctif ferme uniquement le risque de corruption mémoire à la libération ; rendre le contenu d'une telle string réellement correct de bout en bout demanderait une représentation de string à longueur explicite (pas seulement NUL-terminée), un changement bien plus large que ce point.

## Non traité — évalué, jugé plus large qu'un correctif ponctuel

- **Détection de type par heuristique sur la valeur d'un entier** (`read_tag`, `runtime/src/typecheck.rs`) : confirmé **bien plus grave** qu'anticipé — pas juste une mauvaise classification, un **SEGFAULT reproductible** sur du code parfaitement ordinaire :
  ```ocara
  var n:mixed = 1000000
  if n is string { ... }   // SEGFAULT : déréférence l'adresse (1000000 - 8)
  ```
  Toute valeur `mixed` dont la valeur est `>= 65536` et multiple de 4 (une contrainte que N'IMPORTE QUEL entier ordinaire peut remplir par hasard) fait lire une adresse mémoire arbitraire. Une vraie correction demande soit de revoir la représentation des entiers/pointeurs (tagged integers, un bit dédié), soit de vérifier qu'une page est réellement mappée avant de déréférencer (`mincore()` ou équivalent, coût d'un syscall par vérification, spécifique à la plateforme) — les deux dépassent largement un correctif ponctuel. Documenté ici comme une découverte confirmée mais non traitée : à prioriser dans une session dédiée compte tenu de la sévérité (SEGFAULT, pas juste une valeur fausse).

## Fichiers clés

`runtime/src/typecheck.rs` (`TAG_EXCEPTION`, `read_tag`), `runtime/src/exception.rs` (`alloc_exception`), `runtime/src/mutex.rs` (`libc::pthread_mutex_t`), `runtime/Cargo.toml` (dépendance `libc`), `runtime/src/lib.rs` (`__object_free`, `free_str`, non touchés).
