// ─────────────────────────────────────────────────────────────────────────────
// ocara.Mutex — runtime des mutex pour synchronisation thread-safe
//
// Fonctions exportées (convention C) :
//
//   Mutex_init(self_ptr)         → void   constructeur : alloue OcaraMutex
//   Mutex_lock(self_ptr)         → void   verrouille le mutex (bloquant)
//   Mutex_unlock(self_ptr)       → void   déverrouille le mutex
//   Mutex_tryLock(self_ptr)     → i64    tente de verrouiller (non-bloquant, retourne 1 si succès, 0 sinon)
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
//   Utilise pthread_mutex directement via libc pour avoir un contrôle manuel
//   sur lock/unlock sans RAII (nécessaire pour l'API Ocara).
// ─────────────────────────────────────────────────────────────────────────────

use std::alloc::{alloc, dealloc, Layout};

// `libc::pthread_mutex_t` : taille/alignement garantis corrects pour la cible
// de compilation réelle (glibc, musl, macOS, BSD...), contrairement à un
// tableau d'octets hardcodé par plateforme (`[u8;40]` Linux / `[u8;64]`
// macOS/fallback) jamais vérifié contre la vraie taille ABI — une libc dont
// `pthread_mutex_t` dépasserait la taille supposée aurait laissé
// `pthread_mutex_init` écrire hors des bornes de l'allocation (voir
// docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md). Les fonctions
// `pthread_mutex_*` de `libc` sont utilisées directement plutôt que
// redéclarées à la main : garantit que le type du mutex et la signature des
// fonctions qui l'utilisent proviennent de la même source, jamais désynchronisées.
type PthreadMutex = libc::pthread_mutex_t;

/// Struct interne stockée sur le tas (pointeur conservé dans le slot Ocara)
struct OcaraMutex {
    mutex: *mut PthreadMutex,
}

impl Drop for OcaraMutex {
    fn drop(&mut self) {
        unsafe {
            libc::pthread_mutex_destroy(self.mutex);
            dealloc(self.mutex as *mut u8, Layout::new::<PthreadMutex>());
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
        let mutex_ptr = alloc(Layout::new::<PthreadMutex>()) as *mut PthreadMutex;
        libc::pthread_mutex_init(mutex_ptr, std::ptr::null());
        
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
    let result = unsafe { libc::pthread_mutex_lock(m.mutex) };
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
    let result = unsafe { libc::pthread_mutex_unlock(m.mutex) };
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
    let result = unsafe { libc::pthread_mutex_trylock(m.mutex) };
    if result == 0 {
        1 // succès
    } else {
        0 // échec (mutex déjà verrouillé ou erreur)
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
