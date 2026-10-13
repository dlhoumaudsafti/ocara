//! Constantes globales calculées (`const G:int = tick()`) : évaluées une
//! seule fois, en tête de `main`, puis rangées dans un emplacement que
//! chaque fonction relit à son entrée (voir
//! `src/lower/builder.d/functions.rs`). Les constantes littérales n'en ont
//! pas besoin : elles sont recopiées telles quelles.

use std::sync::atomic::AtomicI64;

const SLOTS: usize = 4096;

static VALUES: [AtomicI64; SLOTS] = [const { AtomicI64::new(0) }; SLOTS];

/// Adresse de l'emplacement de la constante n° `idx` (8 octets : entier,
/// flottant, booléen ou pointeur).
#[unsafe(no_mangle)]
pub extern "C" fn __global_const_slot(idx: i64) -> i64 {
    match VALUES.get(idx as usize) {
        Some(slot) => slot.as_ptr() as i64,
        None => {
            eprintln!("ocara: trop de constantes globales calculées (limite {})", SLOTS);
            std::process::abort();
        }
    }
}
