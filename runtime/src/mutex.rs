// ─────────────────────────────────────────────────────────────────────────────
// ocara.Mutex — runtime des mutex pour synchronisation thread-safe
//
// Fonctions exportées (convention C) :
//
//   Mutex_init(self_ptr)         → void   constructeur : alloue OcaraMutex
//   Mutex_lock(self_ptr)         → void   verrouille le mutex (bloquant)
//   Mutex_unlock(self_ptr)       → void   déverrouille le mutex
//   Mutex_tryLock(self_ptr)     → i64    tente de verrouiller (non-bloquant, retourne 1 si succès, 0 sinon)
//   Mutex_withLock(self_ptr, fat_ptr) → void   lock + exécute la closure + unlock garanti (même si elle raise)
//
// Représentation mémoire :
//   Le slot Ocara de 8 octets (alloué par __alloc_obj) stocke un pointeur
//   vers un OcaraMutex boxé sur le tas. Mutex_init y écrit le pointeur,
//   toutes les autres méthodes le lisent via mutex_from_slot().
//
// Note d'usage :
//   Un mutex doit être déverrouillé par le même thread qui l'a verrouillé.
//   Le double-lock du même thread provoque un deadlock.
//   Ne pas oublier d'appeler unlock() après chaque lock() réussi.
//
// Implémentation :
//   Utilise la primitive native de la plateforme directement (pthread_mutex
//   sur Unix, CRITICAL_SECTION sur Windows — voir le module `platform`
//   ci-dessous) pour avoir un contrôle manuel sur lock/unlock sans RAII
//   (nécessaire pour l'API Ocara).
// ─────────────────────────────────────────────────────────────────────────────

use std::alloc::{alloc, dealloc, Layout};

// ── Primitive native par plateforme ─────────────────────────────────────────
// pthread_mutex sur Unix (Linux/macOS/BSD...), CRITICAL_SECTION sur Windows —
// `libc` n'expose AUCUNE fonction `pthread_mutex_*` pour une cible
// `*-windows-*` (confirmé par échec de compilation réel en cross-compilant
// vers `x86_64-pc-windows-gnu`, pas une supposition : Windows n'a pas de
// pthreads natif — MinGW-w64 fournit bien `winpthreads`, mais le crate `libc`
// ne le lie pas). `CRITICAL_SECTION` est l'équivalent natif Windows d'un
// mutex intra-processus récursif, avec exactement les mêmes opérations
// manuelles (init/lock/unlock/trylock/destroy, SANS RAII) qu'un
// `pthread_mutex_t` — même taille/alignement garantis par le type réel de sa
// plateforme (jamais un tableau d'octets hardcodé, voir
// docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md) — aucun changement
// de sémantique pour `ocara.Mutex` des deux côtés.
// `pub(crate)` : réutilisé tel quel par `crate::__alloc_locked_cell`/
// `__locked_cell_get`/`__locked_cell_set` (runtime/src/lib.rs, mutex interne
// aux cellules de capture de closure, jamais exposé à Ocara) — même besoin
// exact (verrou manuel multiplateforme), pas de raison d'avoir deux
// implémentations séparées.
#[cfg(unix)]
pub(crate) mod platform {
    pub type RawMutex = libc::pthread_mutex_t;

    #[inline]
    pub unsafe fn init(m: *mut RawMutex) -> i32 {
        unsafe { libc::pthread_mutex_init(m, std::ptr::null()) }
    }
    #[inline]
    pub unsafe fn lock(m: *mut RawMutex) -> i32 {
        unsafe { libc::pthread_mutex_lock(m) }
    }
    #[inline]
    pub unsafe fn unlock(m: *mut RawMutex) -> i32 {
        unsafe { libc::pthread_mutex_unlock(m) }
    }
    /// Vrai (verrou acquis) si le résultat pthread vaut 0 — même convention
    /// que `pthread_mutex_trylock` (0 = succès, différent de zéro = déjà
    /// verrouillé/erreur).
    #[inline]
    pub unsafe fn try_lock(m: *mut RawMutex) -> bool {
        unsafe { libc::pthread_mutex_trylock(m) == 0 }
    }
    #[inline]
    pub unsafe fn destroy(m: *mut RawMutex) {
        unsafe { libc::pthread_mutex_destroy(m); }
    }
}

#[cfg(windows)]
pub(crate) mod platform {
    pub use windows_sys::Win32::System::Threading::CRITICAL_SECTION as RawMutex;
    use windows_sys::Win32::System::Threading::{
        DeleteCriticalSection, EnterCriticalSection, InitializeCriticalSection,
        LeaveCriticalSection, TryEnterCriticalSection,
    };

    /// `Initialize`/`Enter`/`LeaveCriticalSection` ne peuvent pas échouer côté
    /// API (contrairement à pthread — pas de code d'erreur, seulement un
    /// crash système si les ressources noyau sont épuisées) : ces wrappers
    /// retournent toujours 0 ("succès"), pour que `Mutex_lock`/`Mutex_unlock`
    /// (qui vérifient `result != 0`) n'aient besoin d'aucune branche
    /// spécifique par plateforme.
    #[inline]
    pub unsafe fn init(m: *mut RawMutex) -> i32 {
        unsafe { InitializeCriticalSection(m); }
        0
    }
    #[inline]
    pub unsafe fn lock(m: *mut RawMutex) -> i32 {
        unsafe { EnterCriticalSection(m); }
        0
    }
    #[inline]
    pub unsafe fn unlock(m: *mut RawMutex) -> i32 {
        unsafe { LeaveCriticalSection(m); }
        0
    }
    /// `TryEnterCriticalSection` retourne un booléen Win32 (différent de zéro
    /// = verrou acquis) — convention OPPOSÉE à `pthread_mutex_trylock` (0 =
    /// succès) : normalisé ici en `bool` pour que l'appelant commun
    /// (`Mutex_tryLock`) n'ait pas à connaître cette différence.
    #[inline]
    pub unsafe fn try_lock(m: *mut RawMutex) -> bool {
        unsafe { TryEnterCriticalSection(m) != 0 }
    }
    #[inline]
    pub unsafe fn destroy(m: *mut RawMutex) {
        unsafe { DeleteCriticalSection(m); }
    }
}

/// Struct interne stockée sur le tas (pointeur conservé dans le slot Ocara)
struct OcaraMutex {
    mutex: *mut platform::RawMutex,
}

impl Drop for OcaraMutex {
    fn drop(&mut self) {
        unsafe {
            platform::destroy(self.mutex);
            dealloc(self.mutex as *mut u8, Layout::new::<platform::RawMutex>());
        }
    }
}

/// Lit le pointeur vers OcaraMutex depuis le slot Ocara (8 octets à self_ptr).
#[inline]
unsafe fn mutex_from_slot(self_ptr: i64) -> *mut OcaraMutex {
    unsafe {
        let slot = self_ptr as *const i64;
        *slot as *mut OcaraMutex
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// API publique
// ─────────────────────────────────────────────────────────────────────────────

/// Constructeur : initialise le slot Ocara avec un nouveau OcaraMutex.
#[unsafe(no_mangle)]
pub extern "C" fn Mutex_init(self_ptr: i64) {
    unsafe {
        let mutex_ptr = alloc(Layout::new::<platform::RawMutex>()) as *mut platform::RawMutex;
        platform::init(mutex_ptr);
        
        let m = Box::new(OcaraMutex { mutex: mutex_ptr });
        let raw = Box::into_raw(m) as i64;
        
        // Stocker le pointeur dans le slot alloué par __alloc_obj
        *(self_ptr as *mut i64) = raw;
    }
}

/// Verrouille le mutex. Bloque jusqu'à ce que le mutex soit disponible.
/// Si le mutex est déjà verrouillé par un autre thread, le thread courant attend.
/// Si le même thread tente de verrouiller deux fois, cela provoque un deadlock.
#[unsafe(no_mangle)]
pub extern "C" fn Mutex_lock(self_ptr: i64) {
    let m = unsafe { &*mutex_from_slot(self_ptr) };
    let result = unsafe { platform::lock(m.mutex) };
    if result != 0 {
        unsafe {
            crate::exception::throw_mutex_exception(
                &format!("Failed to lock mutex: error code {}", result),
                101
            );
        }
    }
}

/// Déverrouille le mutex.
/// ATTENTION : doit être appelé par le même thread qui a appelé lock().
/// Appeler unlock() sans lock() préalable est un comportement non défini.
#[unsafe(no_mangle)]
pub extern "C" fn Mutex_unlock(self_ptr: i64) {
    let m = unsafe { &*mutex_from_slot(self_ptr) };
    let result = unsafe { platform::unlock(m.mutex) };
    if result != 0 {
        unsafe {
            crate::exception::throw_mutex_exception(
                &format!("Failed to unlock mutex: error code {} (not owned by current thread?)", result),
                102
            );
        }
    }
}

/// Tente de verrouiller le mutex sans bloquer.
/// Retourne 1 (true) si le verrou a été acquis, 0 (false) sinon.
/// Si succès, un appel à unlock() est requis plus tard.
#[unsafe(no_mangle)]
pub extern "C" fn Mutex_tryLock(self_ptr: i64) -> i64 {
    let m = unsafe { &*mutex_from_slot(self_ptr) };
    if unsafe { platform::try_lock(m.mutex) } {
        1 // succès
    } else {
        0 // échec (mutex déjà verrouillé ou erreur)
    }
}

/// Verrouille le mutex, exécute la closure Ocara fournie (fat pointer
/// {func_ptr, env_ptr}), puis déverrouille systématiquement — y compris si
/// la closure lève une exception (`raise`).
///
/// Problème résolu (voir docs/roadmap.d/memoire-deadlocks-raise.md) :
/// avec `lock()`/`unlock()` manuels, un `raise` entre les deux appels saute
/// `unlock()` (setjmp/longjmp, pas d'unwinding Rust) et laisse le mutex
/// verrouillé pour toujours — tout autre thread en attente dessus est bloqué
/// définitivement. `withLock` encadre l'appel à la closure avec
/// `run_closure_catching` (même mécanisme setjmp/longjmp que
/// `__ocara_try_exec`, voir runtime/src/lib.rs) : si une exception traverse
/// la closure, elle est interceptée ICI, le mutex est déverrouillé, PUIS
/// l'exception est relancée via `__ocara_fail` — elle continue donc de se
/// propager normalement vers le try/catch appelant (ou termine le programme
/// s'il n'y en a pas), mais sans jamais laisser le mutex verrouillé.
#[unsafe(no_mangle)]
pub extern "C" fn Mutex_withLock(self_ptr: i64, fat_ptr: i64) {
    let m = unsafe { &*mutex_from_slot(self_ptr) };
    let lock_result = unsafe { platform::lock(m.mutex) };
    if lock_result != 0 {
        unsafe {
            crate::exception::throw_mutex_exception(
                &format!("Failed to lock mutex: error code {}", lock_result),
                101
            );
        }
    }

    let func_ptr = unsafe { *(fat_ptr as *const i64) };
    let env_ptr  = unsafe { *((fat_ptr as *const i64).add(1)) };

    let outcome = crate::run_closure_catching(func_ptr, env_ptr);

    // Déverrouillage inconditionnel — succès ou exception, c'est tout
    // l'intérêt de withLock() par rapport à lock()/unlock() manuels.
    let unlock_result = unsafe { platform::unlock(m.mutex) };

    if let Err((error_val, error_type)) = outcome {
        // Relancer l'exception d'origine vers l'appelant, maintenant que le
        // mutex est déverrouillé.
        crate::__ocara_fail(error_val, error_type);
    }

    if unlock_result != 0 {
        unsafe {
            crate::exception::throw_mutex_exception(
                &format!("Failed to unlock mutex: error code {} (not owned by current thread?)", unlock_result),
                102
            );
        }
    }
}

/// Libère le mutex (pthread_mutex_destroy + dealloc, via le `Drop` de
/// OcaraMutex ci-dessus — jusqu'ici jamais atteint : rien n'appelait
/// `Box::from_raw` sur le pointeur stocké par `Mutex_init`, donc chaque
/// `use Mutex()` fuyait le wrapper ET le buffer pthread_mutex_t sous-jacent.
/// Comme SQLite_close : usage après `destroy()` est UB (même discipline déjà
/// acceptée pour SQLite/MySQL/SDL).
#[unsafe(no_mangle)]
pub extern "C" fn Mutex_destroy(self_ptr: i64) {
    unsafe {
        let ptr = mutex_from_slot(self_ptr);
        if ptr.is_null() {
            return;
        }
        let _ = Box::from_raw(ptr);
    }
}
