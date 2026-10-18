//! Blocs `array`/`map` et leur verrou.
//!
//! Le collecteur de cycles concurrent (`rc_concurrent`) lit les éléments d'un
//! conteneur pendant que d'autres threads Ocara le modifient : tout accès au
//! `Vec` passe par `with_array`/`with_map`. La section critique ne contient
//! que des opérations sur le `Vec` : jamais de `retain`/`release` (ils
//! peuvent lancer une collecte, qui verrouille à son tour), ni d'exception
//! (`longjmp` sauterait le déverrouillage).

use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct OcaraArray {
    lock: AtomicBool,
    data: Vec<i64>,
}

pub(crate) struct OcaraMap {
    lock: AtomicBool,
    /// Clés stockées comme texte, pour comparer par valeur ; paires gardées
    /// dans l'ordre d'insertion.
    data: Vec<(String, i64)>,
}

impl OcaraArray {
    pub(crate) fn new(data: Vec<i64>) -> Self {
        OcaraArray { lock: AtomicBool::new(false), data }
    }
}

impl OcaraMap {
    pub(crate) fn new() -> Self {
        OcaraMap { lock: AtomicBool::new(false), data: Vec::new() }
    }
}

fn locked<T, R>(lock: &AtomicBool, data: &mut T, f: impl FnOnce(&mut T) -> R) -> R {
    while lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        std::hint::spin_loop();
    }
    let result = f(data);
    lock.store(false, Ordering::Release);
    result
}

/// Accès exclusif aux éléments du tableau `ptr` (bloc `TAG_ARRAY` vivant).
pub(crate) unsafe fn with_array<R>(ptr: i64, f: impl FnOnce(&mut Vec<i64>) -> R) -> R {
    let OcaraArray { lock, data } = unsafe { &mut *(ptr as *mut OcaraArray) };
    locked(lock, data, f)
}

/// Accès exclusif aux paires de la map `ptr` (bloc `TAG_MAP` vivant).
pub(crate) unsafe fn with_map<R>(ptr: i64, f: impl FnOnce(&mut Vec<(String, i64)>) -> R) -> R {
    let OcaraMap { lock, data } = unsafe { &mut *(ptr as *mut OcaraMap) };
    locked(lock, data, f)
}
