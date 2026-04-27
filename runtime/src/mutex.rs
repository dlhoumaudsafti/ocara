// ─────────────────────────────────────────────────────────────────────────────
// ocara.Mutex — runtime des mutex pour synchronisation thread-safe
//
// Fonctions exportées (convention C) :
//
//   Mutex_init(self_ptr)         → void   constructeur : alloue OcaraMutex
//   Mutex_lock(self_ptr)         → void   verrouille le mutex (bloquant)
//   Mutex_unlock(self_ptr)       → void   déverrouille le mutex
//   Mutex_try_lock(self_ptr)     → i64    tente de verrouiller (non-bloquant, retourne 1 si succès, 0 sinon)
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
use std::ptr;

// Types pthread_mutex pour Linux/macOS
#[cfg(target_os = "linux")]
type PthreadMutex = [u8; 40];  // sizeof(pthread_mutex_t) sur Linux x86_64

#[cfg(target_os = "macos")]
type PthreadMutex = [u8; 64];  // sizeof(pthread_mutex_t) sur macOS

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
type PthreadMutex = [u8; 64];  // fallback

extern "C" {
    fn pthread_mutex_init(mutex: *mut PthreadMutex, attr: *const u8) -> i32;
    fn pthread_mutex_lock(mutex: *mut PthreadMutex) -> i32;
    fn pthread_mutex_unlock(mutex: *mut PthreadMutex) -> i32;
    fn pthread_mutex_trylock(mutex: *mut PthreadMutex) -> i32;
    fn pthread_mutex_destroy(mutex: *mut PthreadMutex) -> i32;
}

/// Struct interne stockée sur le tas (pointeur conservé dans le slot Ocara)
struct OcaraMutex {
    mutex: *mut PthreadMutex,
}

impl Drop for OcaraMutex {
    fn drop(&mut self) {
        unsafe {
            pthread_mutex_destroy(self.mutex);
            dealloc(self.mutex as *mut u8, Layout::new::<PthreadMutex>());
        }
    }
}

/// Lit le pointeur vers OcaraMutex depuis le slot Ocara (8 octets à self_ptr).
#[inline]
unsafe fn mutex_from_slot(self_ptr: i64) -> *mut OcaraMutex {
    let slot = self_ptr as *const i64;
    *slot as *mut OcaraMutex
}

// ─────────────────────────────────────────────────────────────────────────────
// API publique
// ─────────────────────────────────────────────────────────────────────────────

/// Constructeur : initialise le slot Ocara avec un nouveau OcaraMutex.
#[no_mangle]
pub extern "C" fn Mutex_init(self_ptr: i64) {
    unsafe {
        let mutex_ptr = alloc(Layout::new::<PthreadMutex>()) as *mut PthreadMutex;
        pthread_mutex_init(mutex_ptr, ptr::null());
        
        let m = Box::new(OcaraMutex { mutex: mutex_ptr });
        let raw = Box::into_raw(m) as i64;
        
        // Stocker le pointeur dans le slot alloué par __alloc_obj
        *(self_ptr as *mut i64) = raw;
    }
}

/// Verrouille le mutex. Bloque jusqu'à ce que le mutex soit disponible.
/// Si le mutex est déjà verrouillé par un autre thread, le thread courant attend.
/// Si le même thread tente de verrouiller deux fois, cela provoque un deadlock.
#[no_mangle]
pub extern "C" fn Mutex_lock(self_ptr: i64) {
    let m = unsafe { &*mutex_from_slot(self_ptr) };
    unsafe {
        pthread_mutex_lock(m.mutex);
    }
}

/// Déverrouille le mutex.
/// ATTENTION : doit être appelé par le même thread qui a appelé lock().
/// Appeler unlock() sans lock() préalable est un comportement non défini.
#[no_mangle]
pub extern "C" fn Mutex_unlock(self_ptr: i64) {
    let m = unsafe { &*mutex_from_slot(self_ptr) };
    unsafe {
        pthread_mutex_unlock(m.mutex);
    }
}

/// Tente de verrouiller le mutex sans bloquer.
/// Retourne 1 (true) si le verrou a été acquis, 0 (false) sinon.
/// Si succès, un appel à unlock() est requis plus tard.
#[no_mangle]
pub extern "C" fn Mutex_try_lock(self_ptr: i64) -> i64 {
    let m = unsafe { &*mutex_from_slot(self_ptr) };
    let result = unsafe { pthread_mutex_trylock(m.mutex) };
    if result == 0 {
        1 // succès
    } else {
        0 // échec (mutex déjà verrouillé ou erreur)
    }
}
