//! Blocs `array`/`map` et leur verrou.
//!
//! Le collecteur de cycles concurrent (`rc_concurrent`) lit les éléments d'un
//! conteneur pendant que d'autres threads Ocara le modifient : tout accès au
//! `Vec` passe par `with_array`/`with_map`. La section critique ne contient
//! que des opérations sur le `Vec` : jamais de `retain`/`release` (ils
//! peuvent lancer une collecte, qui verrouille à son tour), ni d'exception
//! (`longjmp` sauterait le déverrouillage).

use std::collections::hash_map::{DefaultHasher, RandomState};
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};

use indexmap::{Equivalent, IndexMap};

/// Paires d'une map, gardées dans l'ordre d'insertion ; recherche par
/// hachage du texte de la clé (`KeyState`).
pub(crate) type MapData = IndexMap<MapKey, i64, KeyState>;

/// Hachage du texte d'une clé. Un texte décimal canonique (`"42"`, `"-7"`,
/// clé d'une `map<int, V>`) est haché par sa valeur : des clés entières
/// voisines restent voisines dans la table, comme dans un `dict` Python (un
/// hachage du texte dispersait chaque accès : défaut de cache). Tout autre
/// texte passe par SipHash à graine aléatoire (clés venues du réseau).
#[derive(Clone, Default)]
pub(crate) struct KeyState(RandomState);

impl BuildHasher for KeyState {
    type Hasher = KeyHasher;

    fn build_hasher(&self) -> KeyHasher {
        KeyHasher { text: self.0.build_hasher(), int: None }
    }
}

pub(crate) struct KeyHasher {
    text: DefaultHasher,
    int: Option<i64>,
}

impl Hasher for KeyHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.text.write(bytes);
    }

    fn write_i64(&mut self, value: i64) {
        self.int = Some(value);
    }

    fn finish(&self) -> u64 {
        self.int.map_or_else(|| self.text.finish(), int_hash)
    }
}

/// Bits bas : la valeur (position dans la table) ; 7 bits hauts : valeur
/// mélangée (étiquette de groupe de hashbrown).
fn int_hash(value: i64) -> u64 {
    const LOW: u64 = (1 << 57) - 1;
    let x = value as u64;
    (x & LOW) | (x.wrapping_mul(0x9E37_79B9_7F4A_7C15) & !LOW)
}

/// Valeur d'un texte décimal canonique, tel que l'écrit un `int`.
fn canonical_int(text: &str) -> Option<i64> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let canonical = !digits.is_empty() && digits.len() <= 19 && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits.len() == 1 || !digits.starts_with('0')) && !(text.starts_with('-') && digits == "0");
    if canonical { text.parse().ok() } else { None }
}

fn hash_text<H: Hasher>(text: &str, state: &mut H) {
    match canonical_int(text) {
        Some(value) => state.write_i64(value),
        None => state.write(text.as_bytes()),
    }
}

/// Recherche par texte, sans créer de clé : même hachage que `MapKey`.
pub(crate) struct KeyText<'a>(pub &'a str);

impl Hash for KeyText<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_text(self.0, state)
    }
}

impl Equivalent<MapKey> for KeyText<'_> {
    fn equivalent(&self, key: &MapKey) -> bool {
        self.0 == key.text()
    }
}

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

impl Hash for MapKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_text(self.text(), state)
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
        OcaraMap { lock: AtomicBool::new(false), data: MapData::default() }
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
