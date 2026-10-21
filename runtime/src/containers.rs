//! Blocs `array`/`map` et leur verrou.
//!
//! Le collecteur de cycles concurrent (`rc_concurrent`) lit les éléments d'un
//! conteneur pendant que d'autres threads Ocara le modifient : tout accès au
//! `Vec` passe par `with_array`/`with_map`. La section critique ne contient
//! que des opérations sur le `Vec` : jamais de `retain`/`release` (ils
//! peuvent lancer une collecte, qui verrouille à son tour), ni d'exception
//! (`longjmp` sauterait le déverrouillage).

use std::sync::atomic::{AtomicBool, Ordering};

use indexmap::IndexMap;

/// Paires d'une map, gardées dans l'ordre d'insertion ; recherche par
/// hachage du texte de la clé.
pub(crate) type MapData = IndexMap<MapKey, i64>;

/// Clé d'une map : chaîne Ocara retenue par la map, comparée et hachée par
/// son texte (une recherche par `&str` ne crée pas de clé). Rendre les clés
/// (`Map_keys`, `for k has v in m`) les retient au lieu de les recopier.
pub(crate) struct MapKey(i64);

impl MapKey {
    /// Reprend une référence (+1) sur la chaîne `val`.
    pub(crate) unsafe fn owned(val: i64) -> Self {
        MapKey(val)
    }

    pub(crate) fn text(&self) -> &str {
        unsafe { crate::ptr_to_str(self.0) }
    }

    /// La chaîne, retenue pour l'appelant (+1).
    pub(crate) fn share(&self) -> i64 {
        unsafe { crate::rc::retain(self.0) };
        self.0
    }
}

impl Drop for MapKey {
    fn drop(&mut self) {
        unsafe { crate::rc::release(self.0) }
    }
}

impl PartialEq for MapKey {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text()
    }
}

impl Eq for MapKey {}

impl std::hash::Hash for MapKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.text().hash(state)
    }
}

impl std::borrow::Borrow<str> for MapKey {
    fn borrow(&self) -> &str {
        self.text()
    }
}

pub(crate) struct OcaraArray {
    lock: AtomicBool,
    data: Vec<i64>,
}

pub(crate) struct OcaraMap {
    lock: AtomicBool,
    data: MapData,
}

impl OcaraArray {
    pub(crate) fn new(data: Vec<i64>) -> Self {
        OcaraArray { lock: AtomicBool::new(false), data }
    }
}

impl OcaraMap {
    pub(crate) fn new() -> Self {
        OcaraMap { lock: AtomicBool::new(false), data: MapData::new() }
    }
}

fn locked<T, R>(lock: &AtomicBool, data: &mut T, f: impl FnOnce(&mut T) -> R) -> R {
    // Un seul thread exécute du code Ocara : aucune collecte concurrente ni
    // autre thread ne peut lire ce conteneur, le verrou est inutile.
    if !crate::rc::is_multi_threaded() {
        return f(data);
    }
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
pub(crate) unsafe fn with_map<R>(ptr: i64, f: impl FnOnce(&mut MapData) -> R) -> R {
    let OcaraMap { lock, data } = unsafe { &mut *(ptr as *mut OcaraMap) };
    locked(lock, data, f)
}
