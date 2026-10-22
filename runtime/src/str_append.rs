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
#[inline]
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

/// Genre d'une partie de gabarit (voir `__str_build`).
const PART_VALUE: i64 = 0;
const PART_INT: i64 = 1;
const PART_FLOAT: i64 = 2;
const PART_BOOL: i64 = 3;
const PART_ARRAY: i64 = 4;

thread_local! {
    /// Tampon de formatage des gabarits, réutilisé d'un appel à l'autre.
    static TEMPLATE_BUFFER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Appelle `f` avec le tampon de texte du thread, vidé (réutilisé d'un appel
/// à l'autre : aucune allocation pour construire un résultat).
pub(crate) fn with_buffer<R>(f: impl FnOnce(&mut String) -> R) -> R {
    TEMPLATE_BUFFER.with(|buffer| {
        let mut text = buffer.borrow_mut();
        text.clear();
        f(&mut text)
    })
}

/// Texte d'une partie de gabarit qui n'a rien à formater (chaîne, `bool`),
/// `None` sinon.
fn plain_part<'a>(kind: i64, v: i64) -> Option<&'a str> {
    match kind {
        PART_BOOL => Some(if v != 0 { "true" } else { "false" }),
        PART_VALUE if crate::typecheck::__is_string(v) != 0 => Some(unsafe { crate::ptr_to_str(v) }),
        _ => None,
    }
}

/// Nombre de caractères du texte décimal de `n`.
pub(crate) fn int_len(n: i64) -> usize {
    n.unsigned_abs().checked_ilog10().map_or(1, |log| log as usize + 1) + usize::from(n < 0)
}

/// Gabarit `` `a${x}b${y}` `` : `parts` pointe sur `n` paires (genre,
/// valeur) — chaîne ou valeur auto-décrite, `int`, `float` (bits), `bool`,
/// tableau. Fait de chaînes, d'`int` et de `bool`, il est écrit directement
/// dans le résultat, de longueur calculée d'avance ; sinon formaté dans un
/// tampon réutilisé. Une seule allocation dans les deux cas.
#[unsafe(no_mangle)]
pub extern "C" fn __str_build(parts: *const i64, n: i64) -> i64 {
    let parts = unsafe { std::slice::from_raw_parts(parts as *const [i64; 2], n.max(0) as usize) };
    let mut len = 0;
    for &[kind, v] in parts {
        len += match kind {
            PART_INT => int_len(v),
            _ => match plain_part(kind, v) {
                Some(text) => text.len(),
                None => return build_formatted(parts),
            },
        };
    }
    unsafe {
        crate::alloc_str_with(len, |out| {
            let mut at = 0;
            for &[kind, v] in parts {
                let digits = &mut [0; 20];
                let text = match kind {
                    PART_INT => crate::int_text(v, digits),
                    _ => plain_part(kind, v).unwrap_or_default(),
                };
                out[at..at + text.len()].copy_from_slice(text.as_bytes());
                at += text.len();
            }
        })
    }
}

fn build_formatted(parts: &[[i64; 2]]) -> i64 {
    with_buffer(|text| {
        format_parts(parts, text);
        unsafe { crate::alloc_str(text) }
    })
}

fn format_parts(parts: &[[i64; 2]], text: &mut String) {
    for &[kind, v] in parts {
        match kind {
            PART_INT => text.push_str(crate::int_text(v, &mut [0; 20])),
            PART_FLOAT => text.push_str(&crate::float_text(f64::from_bits(v as u64))),
            PART_ARRAY if v == 0 => text.push_str("[]"),
            PART_ARRAY => text.push_str(&crate::ptr_to_display(v, 0)),
            _ => match plain_part(kind, v) {
                Some(plain) => text.push_str(plain),
                None => text.push_str(&crate::val_to_string(v)),
            },
        }
    }
}

/// Appelle `f` avec le texte du gabarit `parts` (voir `__str_build`), sans
/// allouer de chaîne Ocara. Le tampon du thread est sorti le temps de `f` :
/// `f` peut relâcher une valeur, donc lancer une collecte qui formate un
/// autre gabarit.
pub(crate) fn with_template_text<R>(parts: *const i64, n: i64, f: impl FnOnce(&str) -> R) -> R {
    let parts = unsafe { std::slice::from_raw_parts(parts as *const [i64; 2], n.max(0) as usize) };
    let mut text = TEMPLATE_BUFFER.with(|buffer| std::mem::take(&mut *buffer.borrow_mut()));
    text.clear();
    format_parts(parts, &mut text);
    let result = f(&text);
    TEMPLATE_BUFFER.with(|buffer| *buffer.borrow_mut() = text);
    result
}
