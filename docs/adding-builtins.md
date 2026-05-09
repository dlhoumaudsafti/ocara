# Guide : Ajouter une classe builtin à Ocara

Ce guide explique comment créer une nouvelle classe builtin pour Ocara. Les classes builtins sont écrites en Rust et compilées dans le runtime, offrant des performances maximales.

## Vue d'ensemble

Une classe builtin Ocara se compose de :
- **Définition côté compilateur** (`src/builtins/*.rs`) : signatures des méthodes pour le type-checking
- **Implémentation côté runtime** (`runtime/src/*.rs`) : code Rust exporté en C
- **Signatures Cranelift** (`src/codegen/desc.d/*.rs`) : déclarations pour le générateur de code
- **Documentation** (`docs/builtins/*.md`) : guide utilisateur
- **Exemples** (`examples/builtins/*.oc`) : code de démonstration

## Exemple : Classe Counter

Nous allons créer une classe `Counter` simple avec :
- Méthode statique : `Counter::create(initial: int) → Counter`
- Méthodes d'instance : `c.increment()`, `c.decrement()`, `c.value() → int`, `c.reset()`

---

## Étape 1 : Définir la classe (compilateur)

**Fichier : `src/builtins/counter.rs`**

```rust
// ─────────────────────────────────────────────────────────────────────────────
// ocara.Counter — classe builtin pour compteur simple
//
// Méthodes statiques :
//   Counter::create(initial:int) → Counter
//
// Méthodes d'instance :
//   c.increment() → void
//   c.decrement() → void
//   c.value() → int
//   c.reset() → void
//
// Convention runtime : Counter_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

/// Helper pour méthode statique
fn static_m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
    }
}

/// Helper pour méthode d'instance
fn instance(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: false,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // ── Méthode statique ──────────────────────────────────────────────────────
    
    // Counter::create(initial:int) → Counter
    methods.insert("create".into(), static_m(
        vec![("initial", Type::Int)],
        Type::Named("Counter".to_string()),
    ));

    // ── Méthodes d'instance ───────────────────────────────────────────────────

    // c.increment() → void
    methods.insert("increment".into(), instance(
        vec![],
        Type::Void,
    ));

    // c.decrement() → void
    methods.insert("decrement".into(), instance(
        vec![],
        Type::Void,
    ));

    // c.value() → int
    methods.insert("value".into(), instance(
        vec![],
        Type::Int,
    ));

    // c.reset() → void
    methods.insert("reset".into(), instance(
        vec![],
        Type::Void,
    ));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
```

**Points clés :**
- `static_m()` : méthode statique (appelée via `Counter::method()`)
- `instance()` : méthode d'instance (appelée via `c.method()`)
- Le type de retour d'une méthode statique qui crée une instance est `Type::Named("Counter")`
- `is_opaque: false` : la classe est visible mais ses champs internes sont opaques

---

## Étape 2 : Implémenter le runtime

**Fichier : `runtime/src/counter.rs`**

```rust
// ─────────────────────────────────────────────────────────────────────────────
// ocara.Counter — Compteur simple
//
// Fonctions exportées (convention C) :
//
//   Counter_create(initial)     → i64  // retourne pointeur vers OcaraCounter
//   Counter_increment(self_ptr) → void
//   Counter_decrement(self_ptr) → void
//   Counter_value(self_ptr)     → i64
//   Counter_reset(self_ptr)     → void
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::Mutex;

/// Structure interne représentant un compteur
struct OcaraCounter {
    value: Mutex<i64>,
    initial: i64,
}

/// Counter::create(initial:int) → Counter
/// Crée un nouveau compteur avec une valeur initiale
#[no_mangle]
pub unsafe extern "C" fn Counter_create(initial: i64) -> i64 {
    let counter = Box::new(OcaraCounter {
        value: Mutex::new(initial),
        initial,
    });
    Box::into_raw(counter) as i64
}

/// c.increment() → void
/// Incrémente le compteur de 1
#[no_mangle]
pub unsafe extern "C" fn Counter_increment(self_ptr: i64) {
    if self_ptr == 0 {
        return;
    }
    let counter = &*(self_ptr as *const OcaraCounter);
    let mut val = counter.value.lock().unwrap();
    *val += 1;
}

/// c.decrement() → void
/// Décrémente le compteur de 1
#[no_mangle]
pub unsafe extern "C" fn Counter_decrement(self_ptr: i64) {
    if self_ptr == 0 {
        return;
    }
    let counter = &*(self_ptr as *const OcaraCounter);
    let mut val = counter.value.lock().unwrap();
    *val -= 1;
}

/// c.value() → int
/// Retourne la valeur actuelle du compteur
#[no_mangle]
pub unsafe extern "C" fn Counter_value(self_ptr: i64) -> i64 {
    if self_ptr == 0 {
        return 0;
    }
    let counter = &*(self_ptr as *const OcaraCounter);
    *counter.value.lock().unwrap()
}

/// c.reset() → void
/// Réinitialise le compteur à sa valeur initiale
#[no_mangle]
pub unsafe extern "C" fn Counter_reset(self_ptr: i64) {
    if self_ptr == 0 {
        return;
    }
    let counter = &*(self_ptr as *const OcaraCounter);
    let mut val = counter.value.lock().unwrap();
    *val = counter.initial;
}
```

**Points clés :**
- `#[no_mangle]` : préserve le nom de la fonction pour le linker
- `pub unsafe extern "C"` : ABI C pour l'interopérabilité
- Convention de nommage : `<Classe>_<methode>`
- Les méthodes statiques prennent les paramètres directement
- Les méthodes d'instance reçoivent `self_ptr: i64` comme premier paramètre
- Utiliser `Box::into_raw()` pour retourner un pointeur vers une instance
- Toujours vérifier `self_ptr == 0` pour éviter les segfaults

---

## Étape 3 : Enregistrer le module runtime

**Fichier : `runtime/src/lib.rs`**

Ajouter à la section des modules :

```rust
pub mod counter;
```

---

## Étape 4 : Ajouter aux builtins du compilateur

**Fichier : `src/builtins/mod.rs`**

1. Ajouter le module :
```rust
pub mod counter;
```

2. Enregistrer dans `builtin_class()` :
```rust
pub fn builtin_class(name: &str) -> Option<ClassInfo> {
    match name {
        "Counter"     => Some(counter::class()),
        // ... autres classes
```

3. Ajouter à `all_builtins()` :
```rust
pub fn all_builtins() -> Vec<(&'static str, ClassInfo)> {
    vec![
        ("Counter",     counter::class()),
        // ... autres classes
```

---

## Étape 5 : Définir les signatures Cranelift

**Fichier : `src/codegen/desc.d/counter.rs`**

```rust
use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module Counter
pub const COUNTER_BUILTINS: &[BuiltinDesc] = &[
    // Méthode statique
    BuiltinDesc { 
        name: "Counter_create", 
        params: &[clt::I64],               // initial:int
        returns: Some(clt::I64),           // → Counter (pointeur)
        module: Some("Counter") 
    },
    
    // Méthodes d'instance
    BuiltinDesc { 
        name: "Counter_increment", 
        params: &[clt::I64],               // self_ptr
        returns: None,                     // → void
        module: Some("Counter") 
    },
    BuiltinDesc { 
        name: "Counter_decrement", 
        params: &[clt::I64],               // self_ptr
        returns: None,                     // → void
        module: Some("Counter") 
    },
    BuiltinDesc { 
        name: "Counter_value", 
        params: &[clt::I64],               // self_ptr
        returns: Some(clt::I64),           // → int
        module: Some("Counter") 
    },
    BuiltinDesc { 
        name: "Counter_reset", 
        params: &[clt::I64],               // self_ptr
        returns: None,                     // → void
        module: Some("Counter") 
    },
];
```

**Fichier : `src/codegen/desc.d/mod.rs`**

1. Ajouter le module :
```rust
mod counter;
```

2. Exporter la constante :
```rust
pub use counter::COUNTER_BUILTINS;
```

**Fichier : `src/codegen/runtime.rs`**

1. Importer :
```rust
use super::desc::{
    // ... autres imports
    COUNTER_BUILTINS,
};
```

2. Ajouter dans `builtins()` :
```rust
pub fn builtins() -> &'static [BuiltinDesc] {
    BUILTINS_COMBINED.get_or_init(|| {
        let mut all = Vec::new();
        all.extend_from_slice(COUNTER_BUILTINS);
        // ... autres builtins
```

---

## Étape 6 : Enregistrer dans la liste des imports

**Fichier : `src/main.rs`**

Ajouter "Counter" à `OCARA_BUILTINS` :

```rust
const OCARA_BUILTINS: &[&str] = &[
    "Counter", "IO", "Math", // ...
];
```

**Fichier : `tools/ocaracs/src/main.rs`** (même modification)

---

## Étape 7 : Ajouter les dépendances (si nécessaire)

**Fichier : `runtime/Cargo.toml`**

Si votre classe nécessite des crates externes :

```toml
[dependencies]
# Exemple : si Counter avait besoin d'une lib externe
# some_crate = "1.0.0"
```

---

## Étape 8 : Créer la documentation

**Fichier : `docs/builtins/Counter.md`**

```markdown
# ocara.Counter

Classe builtin pour un compteur simple.

## Import

\```ocara
import ocara.Counter
\```

## Création

### `Counter::create(initial: int) → Counter`

Crée un nouveau compteur avec une valeur initiale.

\```ocara
const c:Counter = Counter::create(0)
\```

## Méthodes

### `c.increment() → void`

Incrémente le compteur de 1.

\```ocara
c.increment()
\```

### `c.decrement() → void`

Décrémente le compteur de 1.

\```ocara
c.decrement()
\```

### `c.value() → int`

Retourne la valeur actuelle.

\```ocara
const val:int = c.value()
IO::writeln(\`Counter: \${val}\`)
\```

### `c.reset() → void`

Réinitialise à la valeur initiale.

\```ocara
c.reset()
\```

## Exemple complet

\```ocara
import ocara.Counter
import ocara.IO

init {
    const c:Counter = Counter::create(10)
    
    IO::writeln(\`Initial: \${c.value()}\`)  // 10
    
    c.increment()
    IO::writeln(\`After increment: \${c.value()}\`)  // 11
    
    c.decrement()
    c.decrement()
    IO::writeln(\`After 2 decrements: \${c.value()}\`)  // 9
    
    c.reset()
    IO::writeln(\`After reset: \${c.value()}\`)  // 10
}
\```
```

---

## Étape 9 : Créer un exemple

**Fichier : `examples/builtins/counter.oc`**

```ocara
import ocara.Counter
import ocara.IO

main {
    IO::writeln("=== Counter Example ===")
    IO::writeln("")
    
    // Créer un compteur
    IO::writeln("Creating counter with initial value 5...")
    const counter:Counter = Counter::create(5)
    IO::writeln(`Counter value: ${counter.value()}`)
    IO::writeln("")
    
    // Incrémenter
    IO::writeln("Incrementing 3 times...")
    counter.increment()
    counter.increment()
    counter.increment()
    IO::writeln(`Counter value: ${counter.value()}`)
    IO::writeln("")
    
    // Décrémenter
    IO::writeln("Decrementing 2 times...")
    counter.decrement()
    counter.decrement()
    IO::writeln(`Counter value: ${counter.value()}`)
    IO::writeln("")
    
    // Reset
    IO::writeln("Resetting counter...")
    counter.reset()
    IO::writeln(`Counter value: ${counter.value()}`)
    IO::writeln("")
    
    IO::writeln("=== Done ===")
}
```

---

## Étape 10 : Compiler et tester

```bash
# Compiler Ocara avec la nouvelle classe
make build

# Compiler et exécuter l'exemple
cd examples/builtins
../../target/release/ocara counter.oc -o test_counter
./test_counter
```

---

## Checklist complète

- [ ] Créer `src/builtins/<classe>.rs` avec signatures des méthodes
- [ ] Créer `runtime/src/<classe>.rs` avec implémentation Rust
- [ ] Ajouter `pub mod <classe>;` dans `runtime/src/lib.rs`
- [ ] Ajouter `pub mod <classe>;` dans `src/builtins/mod.rs`
- [ ] Enregistrer dans `builtin_class()` et `all_builtins()` (`src/builtins/mod.rs`)
- [ ] Créer `src/codegen/desc.d/<classe>.rs` avec signatures Cranelift
- [ ] Ajouter `mod <classe>;` et `pub use <classe>::*;` dans `src/codegen/desc.d/mod.rs`
- [ ] Ajouter dans `builtins()` (`src/codegen/runtime.rs`)
- [ ] Ajouter aux imports dans `OCARA_BUILTINS` (`src/main.rs`)
- [ ] Ajouter aux imports dans `OCARA_BUILTINS` (`tools/ocaracs/src/main.rs`)
- [ ] Ajouter dépendances externes si nécessaire (`runtime/Cargo.toml`)
- [ ] Créer documentation `docs/builtins/<Classe>.md`
- [ ] Créer exemple `examples/builtins/<classe>.oc`
- [ ] Compiler avec `make build`
- [ ] Tester l'exemple

---

## Gestion d'erreurs (optionnel)

Si votre classe peut générer des erreurs, créez une exception dédiée :

**Fichier : `src/builtins/exception.rs`**

```rust
pub fn counter_exception_class() -> ClassInfo {
    make_exception_class()
}
```

**Fichier : `src/builtins/mod.rs`**

Dans `builtin_class()` :
```rust
"CounterException" => Some(exception::counter_exception_class()),
```

**Fichier : `runtime/src/exception.rs`**

```rust
pub unsafe fn throw_counter_exception(message: &str, code: i64, source: &str) -> ! {
    let obj_ptr = alloc_exception(message, code, source);
    let type_name = alloc_str("CounterException");
    __ocara_fail(obj_ptr, type_name);
    std::hint::unreachable_unchecked()
}
```

Utilisation dans le runtime :
```rust
if self_ptr == 0 {
    throw_counter_exception("Counter is null", 101, "Counter");
}
```

---

## Bonnes pratiques

1. **Sécurité** : Toujours vérifier `self_ptr == 0` dans les méthodes d'instance
2. **Thread-safety** : Utiliser `Mutex` pour les données mutables partagées
3. **Mémoire** : Utiliser `Box::new()` et `Box::into_raw()` pour allouer sur le tas
4. **Nommage** : Convention `<Classe>_<methode>` pour les fonctions runtime
5. **Documentation** : Documenter tous les codes d'erreur et comportements
6. **Tests** : Créer un exemple complet qui teste toutes les fonctionnalités
7. **Types** : Les pointeurs Ocara sont `i64`, les bools `i64` (0=false, 1=true)
8. **Strings** : Utiliser `alloc_str()` pour créer, `ptr_to_str()` pour lire

---

## Types Ocara → Rust

| Type Ocara | Type Rust | Convention C |
|------------|-----------|--------------|
| `int` | `i64` | `i64` |
| `float` | `f64` | `f64` |
| `bool` | `i64` (0/1) | `i64` |
| `string` | `i64` (ptr) | `i64` |
| `array` | `i64` (ptr) | `i64` |
| `map` | `i64` (ptr) | `i64` |
| `Class` | `i64` (ptr) | `i64` |
| `void` | `()` | `void` |

---

## Ressources

- Exemples existants : `src/builtins/file.rs`, `runtime/src/file.rs`
- Classes avec instances : `HTTPServer`, `SQLite`
- Classes statiques uniquement : `Math`, `System`
- Gestion d'exceptions : `File`, `Directory`

---

**Félicitations !** Vous savez maintenant comment ajouter une classe builtin complète à Ocara. 🚀
