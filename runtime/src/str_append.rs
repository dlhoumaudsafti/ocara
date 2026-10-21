//! Concaténation sans recopie quadratique : `s = s + e` sur une variable
//! `string` est compilé en `s = __str_append(s, e)`, qui reprend la référence
//! de la variable. Une chaîne possédée et non partagée (compte 1) est
//! allongée en place ; sa capacité double quand elle est pleine.
//!
//! La capacité est codée dans les bits hauts du mot `aux` (la longueur
//! occupe les bits bas) : `0` pour une chaîne allouée à sa taille exacte —
//! toutes celles d'`alloc_str` et les littéraux.

use crate::rc;
use crate::typecheck::TAG_STRING_OWNED;

const LEN_MASK: i64 = (1 << 48) - 1;
const CAPACITY_SHIFT: u32 = 48;
const MIN_CAPACITY: usize = 32;

/// Longueur en octets d'une chaîne (littérale ou possédée).
pub(crate) unsafe fn str_len(val: i64) -> usize {
    unsafe { (*((val - 16) as *const i64) & LEN_MASK) as usize }
}

/// Taille du bloc de données d'une chaîne possédée (NUL compris).
pub(crate) unsafe fn str_payload(val: i64) -> usize {
    unsafe {
        let aux = *((val - 16) as *const i64);
        match aux >> CAPACITY_SHIFT {
            0 => (aux & LEN_MASK) as usize + 1,
            exp => 1usize << exp,
        }
    }
}

unsafe fn set_len(val: i64, len: usize, exp: u32) {
    unsafe { *((val - 16) as *mut i64) = len as i64 | ((exp as i64) << CAPACITY_SHIFT) }
}

/// Chaîne possédée contenant `parts` bout à bout, avec de la marge pour les
/// ajouts suivants.
unsafe fn alloc_with_room(parts: &[&[u8]]) -> i64 {
    let len: usize = parts.iter().map(|p| p.len()).sum();
    let exp = (len + 1).max(MIN_CAPACITY).next_power_of_two().trailing_zeros();
    unsafe {
        let val = rc::alloc_block(1 << exp, 0, TAG_STRING_OWNED, false);
        let mut at = val as *mut u8;
        for part in parts {
            std::ptr::copy_nonoverlapping(part.as_ptr(), at, part.len());
            at = at.add(part.len());
        }
        *at = 0;
        set_len(val, len, exp);
        val
    }
}

fn is_owned_string(val: i64) -> bool {
    crate::typecheck::__is_string(val) != 0 && unsafe { crate::typecheck::read_tag(val) } == TAG_STRING_OWNED
}

/// `s + b`, en reprenant la référence `s` (relâchée si elle n'est pas
/// réutilisée) ; rend une chaîne possédée (+1).
#[unsafe(no_mangle)]
pub extern "C" fn __str_append(s: i64, b: i64) -> i64 {
    let tail_owned;
    let tail: &[u8] = if crate::typecheck::__is_string(b) != 0 {
        unsafe { crate::ptr_to_str(b).as_bytes() }
    } else {
        tail_owned = crate::val_to_string(b);
        tail_owned.as_bytes()
    };
    if s != b && is_owned_string(s) && unsafe { rc::count_of(rc::word(s, rc::Kind::String)) } == 1 {
        unsafe {
            let len = str_len(s);
            let needed = len + tail.len() + 1;
            if needed <= str_payload(s) && *((s - 16) as *const i64) >> CAPACITY_SHIFT != 0 {
                let exp = (*((s - 16) as *const i64) >> CAPACITY_SHIFT) as u32;
                std::ptr::copy_nonoverlapping(tail.as_ptr(), (s as *mut u8).add(len), tail.len());
                *(s as *mut u8).add(len + tail.len()) = 0;
                set_len(s, len + tail.len(), exp);
                return s;
            }
            let grown = alloc_with_room(&[crate::ptr_to_str(s).as_bytes(), tail]);
            rc::release(s);
            return grown;
        }
    }
    let head_owned;
    let head: &[u8] = if crate::typecheck::__is_string(s) != 0 {
        unsafe { crate::ptr_to_str(s).as_bytes() }
    } else {
        head_owned = crate::val_to_string(s);
        head_owned.as_bytes()
    };
    let joined = unsafe { alloc_with_room(&[head, tail]) };
    unsafe { rc::release(s) }
    joined
}
