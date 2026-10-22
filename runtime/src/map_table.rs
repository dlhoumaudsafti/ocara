//! Table d'une map : paires gardées dans l'ordre d'insertion (`entries`),
//! retrouvées par un index à adressage ouvert (`slots`). Une case porte
//! l'indice de sa paire et le hachage de 32 bits de sa clé (`map_hash.rs`) :
//! la case d'une autre clé est écartée sans lire sa paire ni le texte de sa
//! clé (avec l'étiquette de 7 bits d'`indexmap`, chaque fausse
//! correspondance coûtait deux défauts de cache), et l'agrandissement replace
//! les cases sans relire les paires. Jusqu'à `SMALL` paires, pas d'index :
//! une clé est cherchée dans les paires elles-mêmes.
//!
//! Tant que toutes ses clés sont des textes d'`int` (`map<int, V>`), la table
//! les range en entiers bruts : ni chaîne allouée, ni texte formaté ou relu.
//! La première autre clé la fait passer aux chaînes (`use_text_keys`) ; le
//! hachage d'un entier étant celui de son texte, l'index ne change pas.
//!
//! Sondage par pas de `STEP` cases : un bloc de clés à suffixe numérique
//! tombé sur un autre se décale en entier, ses clés restent contiguës (par
//! pas de 1, chacune glissait au bout de l'amas : 7 cases sondées en moyenne
//! contre 0,5). Le pas est impair, donc premier avec la taille de l'index :
//! le sondage en parcourt toutes les cases.

use foldhash::fast::RandomState;

use crate::map_hash::{canonical_int, hash_int, hash_text, SUFFIX_BLOCK};

const EMPTY: u64 = 0;
const STEP: usize = SUFFIX_BLOCK as usize + 1;
/// Inverse de `STEP` modulo 2⁶⁴ : nombre de pas entre deux cases.
const STEP_INVERSE: usize = {
    let mut inverse = STEP;
    let mut i = 0;
    while i < 5 {
        inverse = inverse.wrapping_mul(2usize.wrapping_sub(STEP.wrapping_mul(inverse)));
        i += 1;
    }
    inverse
};
const MIN_SLOTS: usize = 8;
/// Jusqu'à ce nombre de paires, pas d'index : une clé est cherchée dans les
/// paires (pointeur puis texte), sans hachage ni case allouée — le cas
/// d'un objet JSON ou d'une petite map de configuration.
const SMALL: usize = 8;

/// `key` : entier brut, ou chaîne Ocara retenue par la table (`text_keys`).
struct Entry {
    key: i64,
    value: i64,
}

/// Clé cherchée ou rangée.
#[derive(Clone, Copy)]
pub(crate) enum Key<'a> {
    Int(i64),
    Text(&'a str),
}

/// Clé d'une paire, empruntée à la table.
#[derive(Clone, Copy)]
pub(crate) enum KeyRef {
    Int(i64),
    Text(i64),
}

impl KeyRef {
    pub(crate) fn with_text<R>(self, f: impl FnOnce(&str) -> R) -> R {
        match self {
            KeyRef::Int(value) => f(crate::int_text(value, &mut [0; 20])),
            KeyRef::Text(text) => f(unsafe { crate::ptr_to_str(text) }),
        }
    }

    /// La clé en chaîne Ocara, possédée par l'appelant (+1).
    pub(crate) fn share(self) -> i64 {
        match self {
            KeyRef::Int(value) => unsafe { crate::alloc_str(crate::int_text(value, &mut [0; 20])) },
            KeyRef::Text(text) => {
                unsafe { crate::rc::retain(text) };
                text
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct MapTable {
    entries: Vec<Entry>,
    slots: Vec<u64>,
    seed: RandomState,
    /// Clés rangées en chaînes ; sinon toutes sont des textes d'`int`,
    /// rangés en entiers bruts.
    text_keys: bool,
}

impl Drop for MapTable {
    fn drop(&mut self) {
        if self.text_keys {
            for entry in &self.entries {
                unsafe { crate::rc::release_string(entry.key) }
            }
        }
    }
}

fn slot_index(slot: u64) -> usize {
    (slot as u32 - 1) as usize
}

fn slot_hash(slot: u64) -> u32 {
    (slot >> 32) as u32
}

fn slot_of(hash: u32, index: usize) -> u64 {
    (u64::from(hash) << 32) | (index as u64 + 1)
}

/// Première case vide à partir de la position de `hash`.
fn free_slot(slots: &[u64], hash: u32) -> usize {
    let mask = slots.len() - 1;
    let mut pos = hash as usize & mask;
    while slots[pos] != EMPTY {
        pos = (pos + STEP) & mask;
    }
    pos
}

/// Nombre de pas de sondage de la case `from` à la case `to`.
fn steps(from: usize, to: usize, mask: usize) -> usize {
    to.wrapping_sub(from).wrapping_mul(STEP_INVERSE) & mask
}

impl MapTable {
    fn hash(&self, key: Key) -> u32 {
        match key {
            Key::Int(value) => hash_int(value),
            Key::Text(text) => hash_text(&self.seed, text),
        }
    }

    /// `key` au rangement de la table ; `None` : texte qui n'est pas celui
    /// d'un `int`, absent d'une table à clés entières.
    fn normalize<'a>(&self, key: Key<'a>) -> Option<Key<'a>> {
        match (self.text_keys, key) {
            (false, Key::Text(text)) => canonical_int(text).map(Key::Int),
            _ => Some(key),
        }
    }

    fn is_key(&self, stored: i64, key: Key) -> bool {
        match key {
            Key::Int(value) if !self.text_keys => stored == value,
            Key::Int(value) => unsafe { crate::ptr_to_str(stored) == crate::int_text(value, &mut [0; 20]) },
            // Même chaîne (clé relevée par `Map_keys`) : sans comparer le texte.
            Key::Text(text) => text.as_ptr() as i64 == stored || unsafe { crate::ptr_to_str(stored) == text },
        }
    }

    /// Case et indice de la paire de clé `key` (normalisée), sinon la case
    /// vide où l'insérer. `slots` ne doit pas être vide.
    fn find(&self, hash: u32, key: Key) -> Result<(usize, usize), usize> {
        let mask = self.slots.len() - 1;
        let mut pos = hash as usize & mask;
        loop {
            let slot = self.slots[pos];
            if slot == EMPTY {
                return Err(pos);
            }
            if slot_hash(slot) == hash && self.is_key(self.entries[slot_index(slot)].key, key) {
                return Ok((pos, slot_index(slot)));
            }
            pos = (pos + STEP) & mask;
        }
    }

    /// Indice de la paire de clé `key` (normalisée) d'une petite table, sans
    /// index.
    fn find_small(&self, key: Key) -> Option<usize> {
        self.entries.iter().position(|entry| self.is_key(entry.key, key))
    }

    /// Case de l'index (aucune pour une petite table) et indice de la paire
    /// de clé `key`.
    fn position(&self, key: Key) -> Option<(Option<usize>, usize)> {
        if self.entries.is_empty() { return None; }
        let key = self.normalize(key)?;
        if self.slots.is_empty() {
            return self.find_small(key).map(|index| (None, index));
        }
        self.find(self.hash(key), key).ok().map(|(pos, index)| (Some(pos), index))
    }

    /// Index construit au passage de `SMALL` paires : chaque clé rangée est
    /// hachée une fois.
    fn build_index(&mut self) {
        let size = ((self.entries.len() + 1) * 2).next_power_of_two().max(MIN_SLOTS);
        self.slots = vec![EMPTY; size];
        for index in 0..self.entries.len() {
            let stored = self.entries[index].key;
            let hash = if self.text_keys {
                hash_text(&self.seed, unsafe { crate::ptr_to_str(stored) })
            } else {
                hash_int(stored)
            };
            let pos = free_slot(&self.slots, hash);
            self.slots[pos] = slot_of(hash, index);
        }
    }

    /// Double l'index ; ses cases relues dans l'ordre sont écrites sur deux
    /// fronts presque séquentiels.
    fn grow(&mut self) {
        let size = (self.slots.len() * 2).max(MIN_SLOTS);
        let old = std::mem::replace(&mut self.slots, vec![EMPTY; size]);
        for slot in old.into_iter().filter(|&slot| slot != EMPTY) {
            let pos = free_slot(&self.slots, slot_hash(slot));
            self.slots[pos] = slot;
        }
    }

    fn use_text_keys(&mut self) {
        for entry in &mut self.entries {
            entry.key = unsafe { crate::alloc_str(crate::int_text(entry.key, &mut [0; 20])) };
        }
        self.text_keys = true;
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn get(&self, key: Key) -> Option<i64> {
        self.position(key).map(|(_, index)| self.entries[index].value)
    }

    pub(crate) fn contains_key(&self, key: Key) -> bool {
        self.position(key).is_some()
    }

    /// Valeur de la paire `index` si sa clé est `key` (parcours dans l'ordre
    /// des clés relevées), sinon recherche de `key`.
    pub(crate) fn value_at(&self, index: usize, key: Key) -> Option<i64> {
        match (self.entries.get(index), self.normalize(key)) {
            (Some(entry), Some(key)) if self.is_key(entry.key, key) => Some(entry.value),
            _ => self.get(key),
        }
    }

    /// Clé de la paire `index`, empruntée : entier brut (`int_key`), sinon
    /// chaîne — la table passe alors aux clés texte.
    pub(crate) fn key_at(&mut self, index: usize, int_key: bool) -> i64 {
        if index >= self.entries.len() { return 0; }
        match (int_key, self.text_keys) {
            (true, true) => canonical_int(unsafe { crate::ptr_to_str(self.entries[index].key) }).unwrap_or(0),
            (false, false) => {
                self.use_text_keys();
                self.entries[index].key
            }
            _ => self.entries[index].key,
        }
    }

    pub(crate) fn value_index(&self, index: usize) -> i64 {
        self.entries.get(index).map_or(0, |entry| entry.value)
    }

    /// Range `value` sous `key` ; rend l'ancienne valeur. `owned` : chaîne
    /// de même texte que `key`, possédée par l'appelant (+1), reprise comme
    /// clé ou relâchée.
    pub(crate) fn insert(&mut self, key: Key, value: i64, owned: Option<i64>) -> Option<i64> {
        let key = match self.normalize(key) {
            Some(key) => key,
            None => {
                self.use_text_keys();
                key
            }
        };
        if self.slots.is_empty() {
            if let Some(index) = self.find_small(key) {
                if let Some(text) = owned { unsafe { crate::rc::release_string(text) } }
                return Some(std::mem::replace(&mut self.entries[index].value, value));
            }
            if self.entries.len() < SMALL {
                let stored = self.stored_key(key, owned);
                self.entries.push(Entry { key: stored, value });
                return None;
            }
            self.build_index();
        }
        let hash = self.hash(key);
        let mut pos = match self.find(hash, key) {
            Ok((_, index)) => {
                if let Some(text) = owned { unsafe { crate::rc::release_string(text) } }
                return Some(std::mem::replace(&mut self.entries[index].value, value));
            }
            Err(free) => Some(free),
        };
        // Au plus une case sur deux occupée : sondages courts.
        if (self.entries.len() + 1) * 2 > self.slots.len() {
            self.grow();
            pos = None;
        }
        let stored = self.stored_key(key, owned);
        let pos = pos.unwrap_or_else(|| free_slot(&self.slots, hash));
        self.slots[pos] = slot_of(hash, self.entries.len());
        self.entries.push(Entry { key: stored, value });
        None
    }

    /// Clé à ranger pour `key` : entier brut, ou chaîne — `owned` reprise,
    /// sinon allouée. `owned` non reprise est relâchée.
    fn stored_key(&self, key: Key, owned: Option<i64>) -> i64 {
        match (key, owned) {
            (Key::Int(value), owned) if !self.text_keys => {
                if let Some(text) = owned { unsafe { crate::rc::release_string(text) } }
                value
            }
            (_, Some(text)) => text,
            (Key::Text(text), None) => unsafe { crate::alloc_str(text) },
            (Key::Int(value), None) => unsafe { crate::alloc_str(crate::int_text(value, &mut [0; 20])) },
        }
    }

    /// Range les paires par texte de clé (ordre de `JSON::decode`), index
    /// reconstruit s'il existe.
    pub(crate) fn sort_by_key_text(&mut self) {
        if self.text_keys {
            self.entries.sort_by(|a, b| unsafe { crate::ptr_to_str(a.key).cmp(crate::ptr_to_str(b.key)) });
        } else {
            let text = |key: i64| crate::int_text(key, &mut [0; 20]).to_string();
            self.entries.sort_by_cached_key(|entry| text(entry.key));
        }
        if !self.slots.is_empty() {
            self.build_index();
        }
    }

    /// Retire la paire de clé `key` en gardant l'ordre des autres ; rend sa
    /// valeur.
    pub(crate) fn remove(&mut self, key: Key) -> Option<i64> {
        let (pos, index) = self.position(key)?;
        if let Some(pos) = pos {
            self.clear_slot(pos);
        }
        let Entry { key, value } = self.entries.remove(index);
        if self.text_keys {
            unsafe { crate::rc::release_string(key) }
        }
        if pos.is_some() && index < self.entries.len() {
            for slot in &mut self.slots {
                if *slot != EMPTY && slot_index(*slot) > index {
                    *slot -= 1;
                }
            }
        }
        Some(value)
    }

    /// Vide la case `hole` en remontant les cases suivantes de la suite de
    /// sondage qui peuvent l'occuper (sans case tombale).
    fn clear_slot(&mut self, mut hole: usize) {
        let mask = self.slots.len() - 1;
        let mut pos = hole;
        loop {
            pos = (pos + STEP) & mask;
            let slot = self.slots[pos];
            if slot == EMPTY { break; }
            let home = slot_hash(slot) as usize & mask;
            if steps(home, pos, mask) >= steps(hole, pos, mask) {
                self.slots[hole] = slot;
                hole = pos;
            }
        }
        self.slots[hole] = EMPTY;
    }

    /// Nombre moyen de pas d'une case depuis sa position d'origine.
    #[cfg(test)]
    pub(crate) fn mean_displacement(&self) -> f64 {
        if self.slots.is_empty() { return 0.0; }
        let mask = self.slots.len() - 1;
        let total: usize = self.slots.iter().enumerate().filter(|&(_, &slot)| slot != EMPTY)
            .map(|(pos, &slot)| steps(slot_hash(slot) as usize, pos, mask)).sum();
        total as f64 / self.entries.len().max(1) as f64
    }

    #[cfg(test)]
    pub(crate) fn has_text_keys(&self) -> bool {
        self.text_keys
    }

    fn key_ref(&self, key: i64) -> KeyRef {
        if self.text_keys { KeyRef::Text(key) } else { KeyRef::Int(key) }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (KeyRef, i64)> + '_ {
        self.entries.iter().map(|entry| (self.key_ref(entry.key), entry.value))
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = KeyRef> + '_ {
        self.entries.iter().map(|entry| self.key_ref(entry.key))
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = i64> + '_ {
        self.entries.iter().map(|entry| entry.value)
    }
}
