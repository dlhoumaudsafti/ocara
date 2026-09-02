// ─────────────────────────────────────────────────────────────────────────────
// ocara.Thread — runtime des threads
//
// Fonctions exportées (convention C) :
//
//   Thread_init(self_ptr)           → void   constructeur : alloue OcaraThread
//   Thread_run(self_ptr, fat_ptr)   → void   lance le thread avec une closure
//   Thread_join(self_ptr)           → void   attend la fin du thread
//   Thread_detach(self_ptr)         → void   détache (fire-and-forget)
//   Thread_id(self_ptr)             → i64    ID du thread (assigné à la création)
//   Thread_sleep(ms)                → void   pause en millisecondes (statique)
//   Thread_currentId()             → i64    ID du thread courant (statique)
//
// Représentation mémoire :
//   Le slot Ocara de 8 octets (alloué par __alloc_obj) stocke un pointeur
//   vers un OcaraThread boxé sur le tas. Thread_init y écrit le pointeur,
//   toutes les autres méthodes le lisent via thread_from_slot().
//
// Note sécurité concurrente :
//   Les shared cells des closures (heap_promoted) ne sont pas protégées par
//   un mutex. Des accès concurrents non-synchronisés à des variables partagées
//   entre threads constituent un data race — non défini. Utiliser ocara.Mutex
//   (à implémenter) pour la synchronisation.
// ─────────────────────────────────────────────────────────────────────────────

use std::cell::Cell;
use std::sync::atomic::{AtomicI64, Ordering};

// Compteur global d'IDs de threads (thread 0 = main implicite)
static NEXT_ID: AtomicI64 = AtomicI64::new(1);

// ID du thread courant (thread-local)
thread_local! {
    static CURRENT_THREAD_ID: Cell<i64> = const { Cell::new(0) };
}

/// Signature d'une closure Ocara : fn(env_ptr: i64) -> i64
type OcaraClosureFn = unsafe extern "C" fn(i64) -> i64;

/// Struct interne stockée sur le tas (pointeur conservé dans le slot Ocara)
struct OcaraThread {
    id:     i64,
    handle: Option<std::thread::JoinHandle<()>>,
}

/// Lit le pointeur vers OcaraThread depuis le slot Ocara (8 octets à self_ptr).
#[inline]
unsafe fn thread_from_slot(self_ptr: i64) -> *mut OcaraThread {
    unsafe {
        let slot = self_ptr as *const i64;
        *slot as *mut OcaraThread
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Wrapper permettant d'envoyer les pointeurs bruts entre threads.
// Safety : la closure Ocara et son env sont alloués sur le tas (heap_promoted)
// et vivent aussi longtemps que le thread. L'appelant est responsable de la
// synchronisation des accès concurrents aux données partagées.
// ─────────────────────────────────────────────────────────────────────────────
struct SendClosure {
    func_ptr:  i64,
    env_ptr:   i64,
    thread_id: i64,
}
unsafe impl Send for SendClosure {}

// ─────────────────────────────────────────────────────────────────────────────
// API publique
// ─────────────────────────────────────────────────────────────────────────────

/// Constructeur : initialise le slot Ocara avec un nouveau OcaraThread.
#[unsafe(no_mangle)]
pub extern "C" fn Thread_init(self_ptr: i64) {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let t  = Box::new(OcaraThread { id, handle: None });
    let raw = Box::into_raw(t) as i64;
    // Stocker le pointeur dans le slot alloué par __alloc_obj
    unsafe { *(self_ptr as *mut i64) = raw; }
}

/// Lance le thread avec une closure Ocara (fat pointer {func_ptr, env_ptr}).
/// Le thread n'est pas encore joinable avant cet appel.
#[unsafe(no_mangle)]
pub extern "C" fn Thread_run(self_ptr: i64, fat_ptr: i64) {
    let t = unsafe { &mut *thread_from_slot(self_ptr) };

    // Lire func_ptr et env_ptr depuis le fat pointer
    let func_ptr = unsafe { *(fat_ptr as *const i64) };
    let env_ptr  = unsafe { *((fat_ptr as *const i64).add(1)) };

    let sc = SendClosure { func_ptr, env_ptr, thread_id: t.id };

    let handle = match std::thread::Builder::new().spawn(move || {
        // Initialiser l'ID du thread courant pour ce thread
        CURRENT_THREAD_ID.with(|c| c.set(sc.thread_id));
        let f: OcaraClosureFn = unsafe { std::mem::transmute(sc.func_ptr as usize) };
        unsafe { f(sc.env_ptr) };
    }) {
        Ok(h) => h,
        Err(e) => unsafe {
            crate::exception::throw_thread_exception(
                &format!("Failed to spawn thread: {}", e),
                101
            );
        }
    };

    t.handle = Some(handle);
}

/// Attend que le thread se termine (bloquant), puis libère le wrapper
/// OcaraThread (jusqu'ici jamais libéré, même sur ce chemin nominal — voir
/// Thread_detach ci-dessous pour la même correction). Comme pour SQLite/Mutex,
/// tout appel sur ce Thread APRÈS join() est UB (le slot pointe sur de la
/// mémoire libérée) — discipline déjà acceptée ailleurs dans ce runtime.
///
/// Piège : les exceptions Ocara utilisent setjmp/longjmp (pas le unwinding
/// Rust) — un longjmp saute par-dessus le Drop de fin de scope. Le wrapper
/// doit donc être libéré EXPLICITEMENT avant throw_thread_exception sur le
/// chemin panic, sinon cette libération serait elle-même sautée.
#[unsafe(no_mangle)]
pub extern "C" fn Thread_join(self_ptr: i64) {
    unsafe {
        let ptr = thread_from_slot(self_ptr);
        if ptr.is_null() {
            return;
        }
        let mut t = Box::from_raw(ptr);
        if let Some(h) = t.handle.take() {
            if let Err(_) = h.join() {
                drop(t); // avant le throw — voir la note ci-dessus
                crate::exception::throw_thread_exception(
                    "Thread panicked during execution",
                    102
                );
            }
        }
        // chemin normal : `t` est droppé ici en sortant du bloc
    }
}

/// Détache le thread (fire-and-forget) et libère le wrapper OcaraThread.
/// Après detach(), tout appel sur ce Thread est UB (même remarque que join()).
#[unsafe(no_mangle)]
pub extern "C" fn Thread_detach(self_ptr: i64) {
    unsafe {
        let ptr = thread_from_slot(self_ptr);
        if ptr.is_null() {
            return;
        }
        // Dropping le JoinHandle (via le Drop implicite de OcaraThread ici)
        // détache le thread — Box::from_raw libère aussi le wrapper.
        let _ = Box::from_raw(ptr);
    }
}

/// Retourne l'ID unique du thread (assigné à la création).
#[unsafe(no_mangle)]
pub extern "C" fn Thread_id(self_ptr: i64) -> i64 {
    let t = unsafe { &*thread_from_slot(self_ptr) };
    t.id
}

/// Pause le thread courant pendant `ms` millisecondes.
#[unsafe(no_mangle)]
pub extern "C" fn Thread_sleep(ms: i64) {
    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
}

/// Retourne l'ID du thread courant (0 pour le thread principal).
#[unsafe(no_mangle)]
pub extern "C" fn Thread_currentId() -> i64 {
    CURRENT_THREAD_ID.with(|c| c.get())
}
