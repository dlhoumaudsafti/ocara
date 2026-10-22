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

/// Chaîne possédée de `head` puis `extra` octets à remplir par l'appelant
/// (NUL déjà posé), avec de la marge pour les ajouts suivants.
unsafe fn alloc_with_room(head: &[u8], extra: usize) -> i64 {
    let len = head.len() + extra;
    let exp = (len + 1).max(MIN_CAPACITY).next_power_of_two().trailing_zeros();
    unsafe {
        let val = rc::alloc_block(1 << exp, 0, TAG_STRING_OWNED, false);
        std::ptr::copy_nonoverlapping(head.as_ptr(), val as *mut u8, head.len());
        *(val as *mut u8).add(len) = 0;
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
    // `s + s` : la queue est lue dans `s`, qui ne doit pas bouger avant la copie.
    unsafe { append_with(s, tail.len(), s != b, |out| out.copy_from_slice(tail)) }
}

/// `s += `…${x}…`` : les parties du gabarit (voir `__str_build`) écrites
/// directement au bout de `s`, sans chaîne temporaire pour le gabarit.
#[unsafe(no_mangle)]
pub extern "C" fn __str_append_tpl(s: i64, parts: *const i64, n: i64) -> i64 {
    let pairs = unsafe { std::slice::from_raw_parts(parts as *const [i64; 2], n.max(0) as usize) };
    if let Some(len) = plain_len(pairs) {
        // `s += `${s}…`` : une partie lue dans `s`, qui ne doit pas bouger.
        let in_place = !pairs.iter().any(|&[_, v]| v == s);
        return unsafe { append_with(s, len, in_place, |out| write_plain(pairs, out)) };
    }
    with_template_text(parts, n, |text| unsafe { append_with(s, text.len(), true, |out| out.copy_from_slice(text.as_bytes())) })
}

/// `s` (référence reprise) suivie de `extra` octets écrits par `fill`. Une
/// chaîne possédée non partagée (et `in_place`) est allongée sur place :
/// dans sa capacité, sinon par agrandissement du bloc (capacité doublée) ;
/// sinon une nouvelle chaîne reçoit `s` puis la suite, et `s` est relâchée.
unsafe fn append_with(s: i64, extra: usize, in_place: bool, fill: impl FnOnce(&mut [u8])) -> i64 {
    unsafe {
        if in_place && is_owned_string(s) && rc::count_of(rc::word(s, rc::Kind::String)) == 1 {
            let len = str_len(s);
            let needed = len + extra + 1;
            let exp = (*((s - 16) as *const i64) >> CAPACITY_SHIFT) as u32;
            let target = if needed <= str_payload(s) && exp != 0 {
                Some((s, exp))
            } else {
                let exp = needed.max(MIN_CAPACITY).next_power_of_two().trailing_zeros();
                rc::realloc_block(s, str_payload(s), 1 << exp).map(|grown| (grown, exp))
            };
            if let Some((target, exp)) = target {
                fill(std::slice::from_raw_parts_mut((target as *mut u8).add(len), extra));
                *(target as *mut u8).add(len + extra) = 0;
                set_len(target, len + extra, exp);
                return target;
            }
        }
        let head_owned;
        let head: &[u8] = if crate::typecheck::__is_string(s) != 0 {
            crate::ptr_to_str(s).as_bytes()
        } else {
            head_owned = crate::val_to_string(s);
            head_owned.as_bytes()
        };
        let joined = alloc_with_room(head, extra);
        fill(std::slice::from_raw_parts_mut((joined as *mut u8).add(head.len()), extra));
        rc::release(s);
        joined
    }
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
    match plain_len(parts) {
        Some(len) => unsafe { crate::alloc_str_with(len, |out| write_plain(parts, out)) },
        None => build_formatted(parts),
    }
}

/// Longueur d'un gabarit fait de chaînes, d'`int` et de `bool` ; `None` s'il
/// a une partie à formater (flottant, tableau, valeur quelconque).
fn plain_len(parts: &[[i64; 2]]) -> Option<usize> {
    parts.iter().map(|&[kind, v]| match kind {
        PART_INT => Some(int_len(v)),
        _ => plain_part(kind, v).map(str::len),
    }).sum()
}

/// Écrit dans `out` (de longueur `plain_len`) les parties d'un gabarit simple.
fn write_plain(parts: &[[i64; 2]], out: &mut [u8]) {
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
