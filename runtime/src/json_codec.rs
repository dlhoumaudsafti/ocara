//! Conversion directe entre valeurs Ocara et texte JSON (`JSON::encode`,
//! `JSON::decode`) par les traits de serde, sans arbre `serde_json::Value`
//! intermédiaire (un `BTreeMap` et une copie de chaque clé par objet, puis
//! leur libération). Même texte et mêmes valeurs qu'en passant par `Value` :
//! clés d'objet triées à l'encodage comme au décodage, dernière valeur d'une
//! clé en double, `null` pour un flottant non fini.

use std::fmt;

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::*;

/// Texte JSON de `val` (+1). `shape` : voir `JSON_encode`.
pub(crate) fn encode(val: i64, shape: i64) -> i64 {
    str_append::with_buffer(|out| {
        // serde_json n'écrit que de l'UTF-8 valide.
        if serde_json::to_writer(unsafe { out.as_mut_vec() }, &Encoded { val, shape }).is_err() {
            out.clear();
            out.push_str("null");
        }
        unsafe { alloc_str(out) }
    })
}

/// Valeur Ocara (+1) décrite par `text`, `null` (0) si le texte n'est pas du
/// JSON valide ; rien de ce qui a été construit avant l'erreur ne reste.
pub(crate) fn decode(text: &str) -> i64 {
    let mut de = serde_json::Deserializer::from_str(text);
    match Decoded::deserialize(&mut de) {
        Ok(Decoded(val)) if de.end().is_ok() => val,
        Ok(Decoded(val)) => {
            unsafe { rc::release(val) };
            0
        }
        Err(_) => 0,
    }
}

/// `shape` = `kind | depth << 8` (voir `static_leaf_shape` côté lowering) :
/// type de feuille d'un conteneur concret (1/2/3 = int/float/bool, 0 =
/// `mixed`), le même à toute profondeur. Une feuille brute est convertie
/// sans lecture de tag — un entier comme 70000 ressemble à un pointeur
/// aligné ; seule une valeur `mixed` est déballée (cellule ou en ligne).
struct Encoded {
    val: i64,
    shape: i64,
}

impl Serialize for Encoded {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let (val, leaf_kind, depth) = (self.val, self.shape & 0xff, self.shape >> 8);
        if leaf_kind != 0 && depth == 0 {
            return raw_leaf(val, leaf_kind, s);
        }
        let child = if leaf_kind == 0 { 0 } else { leaf_kind | (depth - 1) << 8 };
        if val == 0 {
            // `null` seulement pour `mixed` : sinon 0 est une valeur normale.
            return match leaf_kind {
                1 => s.serialize_i64(0),
                2 => s.serialize_f64(0.0),
                3 => s.serialize_bool(false),
                _ => s.serialize_unit(),
            };
        }
        if leaf_kind == 0 {
            if is_float_box(val) {
                return s.serialize_f64(unsafe { unbox_float(val) });
            }
            if is_bool_box(val) {
                return s.serialize_bool(unsafe { unbox_bool(val) });
            }
            if is_int_box(val) {
                return s.serialize_i64(unsafe { unbox_int(val) });
            }
        }
        match get_value_type(val) {
            // Un booléen de `mixed` est toujours boxé (traité plus haut) :
            // une valeur brute y est un entier, `1` compris.
            1 => raw_leaf(val, leaf_kind, s),
            4 => s.serialize_str(unsafe { ptr_to_str(val) }),
            5 => {
                // Forme inconnue du compilateur (tableau concret dans un
                // `mixed`) : type des éléments bruts porté par le tableau.
                let child = if child == 0 { rc::raw_kind(val) } else { child };
                let items = unsafe { array_data(val) };
                let mut seq = s.serialize_seq(Some(items.len()))?;
                for val in items {
                    seq.serialize_element(&Encoded { val, shape: child })?;
                }
                seq.end()
            }
            6 => {
                let child = if child == 0 { rc::raw_kind(val) } else { child };
                // Clés retenues (pas recopiées), triées par leur texte.
                let mut entries = unsafe { map_shared_entries(val) };
                entries.sort_unstable_by(|a, b| unsafe { ptr_to_str(a.0).cmp(ptr_to_str(b.0)) });
                let result = serialize_entries(s, &entries, child);
                for (key, _) in entries {
                    unsafe { rc::release(key) }
                }
                result
            }
            _ => s.serialize_unit(),
        }
    }
}

fn serialize_entries<S: Serializer>(s: S, entries: &[(i64, i64)], child: i64) -> Result<S::Ok, S::Error> {
    let mut map = s.serialize_map(Some(entries.len()))?;
    for &(key, val) in entries {
        map.serialize_entry(unsafe { ptr_to_str(key) }, &Encoded { val, shape: child })?;
    }
    map.end()
}

fn raw_leaf<S: Serializer>(val: i64, leaf_kind: i64, s: S) -> Result<S::Ok, S::Error> {
    match leaf_kind {
        2 => s.serialize_f64(f64::from_bits(val as u64)),
        3 => s.serialize_bool(val != 0),
        _ => s.serialize_i64(val),
    }
}

/// Valeur décodée (+1), logée dans un `mixed` : booléen et flottant en
/// cellule, entier par `box_int_if_needed`.
struct Decoded(i64);

impl<'de> Deserialize<'de> for Decoded {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        de.deserialize_any(DecodedVisitor)
    }
}

struct DecodedVisitor;

impl<'de> Visitor<'de> for DecodedVisitor {
    type Value = Decoded;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("une valeur JSON")
    }

    fn visit_unit<E>(self) -> Result<Decoded, E> {
        Ok(Decoded(0))
    }

    fn visit_bool<E>(self, b: bool) -> Result<Decoded, E> {
        Ok(Decoded(__box_bool(i64::from(b))))
    }

    fn visit_i64<E>(self, n: i64) -> Result<Decoded, E> {
        Ok(Decoded(box_int_if_needed(n)))
    }

    /// Au-delà de `i64::MAX`, un flottant (comme `Value::as_f64`).
    fn visit_u64<E>(self, n: u64) -> Result<Decoded, E> {
        Ok(Decoded(i64::try_from(n).map_or_else(|_| __box_float((n as f64).to_bits() as i64), box_int_if_needed)))
    }

    fn visit_f64<E>(self, f: f64) -> Result<Decoded, E> {
        Ok(Decoded(__box_float(f.to_bits() as i64)))
    }

    fn visit_str<E>(self, s: &str) -> Result<Decoded, E> {
        Ok(Decoded(unsafe { alloc_str(s) }))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Decoded, A::Error> {
        let array = __array_new();
        loop {
            match seq.next_element::<Decoded>() {
                Ok(Some(Decoded(val))) => array_push_owned(array, val),
                Ok(None) => return Ok(Decoded(array)),
                Err(e) => {
                    unsafe { rc::release(array) };
                    return Err(e);
                }
            }
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Decoded, A::Error> {
        let map = __map_new();
        loop {
            // Clé puis valeur lues séparément : une valeur invalide ne doit pas
            // perdre la clé déjà décodée.
            let key = match access.next_key::<Decoded>() {
                Ok(Some(Decoded(key))) => key,
                Ok(None) => break,
                Err(e) => {
                    unsafe { rc::release(map) };
                    return Err(e);
                }
            };
            match access.next_value::<Decoded>() {
                // Clé en double : la dernière valeur remplace la précédente.
                Ok(Decoded(val)) => map_set_owned_key(map, key, val),
                Err(e) => {
                    unsafe {
                        rc::release(key);
                        rc::release(map);
                    }
                    return Err(e);
                }
            }
        }
        // Comme l'objet de `Value` (`BTreeMap`) : clés triées.
        unsafe { containers::with_map(map, |data| data.sort_by_key_text()) };
        Ok(Decoded(map))
    }
}
