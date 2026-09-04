// ─────────────────────────────────────────────────────────────────────────────
// Type checking runtime pour le narrowing 'is Type'
//
// Toutes les allocations heap (string, array, map, objet, fat-pointer) sont
// précédées d'un header de 8 octets contenant un tag de type.
// Le pointeur retourné pointe APRES ce header (offset +8).
// Ainsi `read_tag(val)` lit *(val - 8) pour récupérer le tag.
//
// Schéma mémoire :
//   [tag: i64]  [données...]
//   ^           ^
//   raw         val (returned to Ocara code)
// ─────────────────────────────────────────────────────────────────────────────

/// Tags stockés dans le header à l'offset (val - 8).
pub(crate) const TAG_STRING:   i64 = 1;
pub(crate) const TAG_ARRAY:    i64 = 2;
pub(crate) const TAG_MAP:      i64 = 3;
pub(crate) const TAG_OBJECT:   i64 = 4;
pub(crate) const TAG_FUNCTION: i64 = 5;
/// Comme `TAG_STRING`, mais spécifiquement pour une string ALLOUÉE SUR LE TAS
/// par `alloc_str` (concaténation, `String::*`, lecture fichier, ...) — par
/// opposition à un littéral (`"foo"`) compilé en constante `.rodata` avec le
/// header `TAG_STRING` (voir `src/codegen/emit.d/emitter.rs`/`constants.rs`).
/// Les deux tags désignent une "vraie" string au sens du langage (`__is_string`
/// répond vrai pour les deux) ; seul `TAG_STRING_OWNED` peut être libéré en
/// toute sécurité (`dealloc`) — un littéral n'a jamais été `alloc()`.
/// Introduit pour `scoped`/`consumed string` (voir `crate::__value_free`) :
/// avant ce tag, libérer une `scoped string` plantait dès qu'elle contenait
/// un littéral (`munmap_chunk(): invalid pointer`).
pub(crate) const TAG_STRING_OWNED: i64 = 6;

const PTR_THRESHOLD: i64 = 65536;

/// Lit le tag de type stocké 8 octets AVANT le pointeur.
/// Retourne 0 si val n'est pas un pointeur heap plain (null, petit entier, boxed).
#[inline]
pub(crate) unsafe fn read_tag(val: i64) -> i64 {
    unsafe {
        // Un pointeur heap plain : >= PTR_THRESHOLD ET bits 1:0 == 0
        if val < PTR_THRESHOLD || (val & 3) != 0 { return 0; }
        *((val - 8) as *const i64)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __is_null(val: i64) -> i64 {
    if val == 0 { 1 } else { 0 }
}

/// int : val != 0 && val < PTR_THRESHOLD (entier direct, pas un pointeur)
#[unsafe(no_mangle)]
pub extern "C" fn __is_int(val: i64) -> i64 {
    if val != 0 && val.abs() < PTR_THRESHOLD { 1 } else { 0 }
}

/// float : détecté via le tag de boxing (bits 1:0 = 01, utilisé par __box_float).
/// Ne fonctionne que pour les floats stockés dans un contexte `mixed`.
/// Les floats directs (f64 bits dans i64) ne sont pas détectables sans boxing.
#[unsafe(no_mangle)]
pub extern "C" fn __is_float(val: i64) -> i64 {
    if val >= PTR_THRESHOLD && (val & 3) == 1 { 1 } else { 0 }
}

/// bool : val == 0 ou val == 1.
/// ⚠ Peut confondre avec les int 0 et 1.
#[unsafe(no_mangle)]
pub extern "C" fn __is_bool(val: i64) -> i64 {
    if val == 0 || val == 1 { 1 } else { 0 }
}

/// string : tag == TAG_STRING (littéral) ou TAG_STRING_OWNED (tas) dans le
/// header — les deux sont des strings au sens du langage, seule leur
/// "libérabilité" diffère (voir `TAG_STRING_OWNED`).
#[unsafe(no_mangle)]
pub extern "C" fn __is_string(val: i64) -> i64 {
    let tag = unsafe { read_tag(val) };
    if tag == TAG_STRING || tag == TAG_STRING_OWNED { 1 } else { 0 }
}

/// array : tag == TAG_ARRAY dans le header
#[unsafe(no_mangle)]
pub extern "C" fn __is_array(val: i64) -> i64 {
    if unsafe { read_tag(val) } == TAG_ARRAY { 1 } else { 0 }
}

/// map : tag == TAG_MAP dans le header
#[unsafe(no_mangle)]
pub extern "C" fn __is_map(val: i64) -> i64 {
    if unsafe { read_tag(val) } == TAG_MAP { 1 } else { 0 }
}

/// object (instance de classe) : tag == TAG_OBJECT dans le header
#[unsafe(no_mangle)]
pub extern "C" fn __is_object(val: i64) -> i64 {
    if unsafe { read_tag(val) } == TAG_OBJECT { 1 } else { 0 }
}

/// Function (fat pointer) : tag == TAG_FUNCTION dans le header
#[unsafe(no_mangle)]
pub extern "C" fn __is_function(val: i64) -> i64 {
    if unsafe { read_tag(val) } == TAG_FUNCTION { 1 } else { 0 }
}
