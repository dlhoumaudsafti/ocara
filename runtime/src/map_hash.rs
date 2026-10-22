//! Hachage de 32 bits d'une clé de map (voir `map_table.rs`).
//!
//! Un entier — clé brute d'une `map<int, V>` ou texte décimal canonique
//! (`"42"`) — est placé par sa valeur : des clés entières voisines restent
//! voisines dans la table, comme dans un `dict` Python. Le texte d'un entier
//! et l'entier lui-même ont le même hachage : la table passe des clés
//! entières aux clés texte sans toucher à son index. Un texte terminé par des
//! chiffres (`"k42"`, `"user_7"`) garde cette localité par blocs de
//! `SUFFIX_BLOCK` valeurs : préfixe et numéro de bloc passent par foldhash,
//! seule la position dans le bloc suit la valeur (comme la table sans graine
//! de PHP, mais un attaquant ne peut aligner plus d'un bloc). Tout autre
//! texte passe par foldhash à graine aléatoire (clés venues du réseau).

use std::hash::{BuildHasher, Hasher};

use foldhash::fast::RandomState;

pub(crate) const SUFFIX_BLOCK: u64 = 256;

/// Préfixe, nombre et valeur des chiffres finaux d'un texte (au plus 18),
/// lus en une passe depuis la fin.
fn numeric_suffix(text: &str) -> Option<(&str, usize, u64)> {
    let bytes = text.as_bytes();
    let (mut start, mut value, mut scale) = (bytes.len(), 0u64, 1u64);
    while start > 0 && bytes[start - 1].is_ascii_digit() {
        if bytes.len() - start == 18 { return None; }
        start -= 1;
        value += u64::from(bytes[start] - b'0') * scale;
        scale = scale.wrapping_mul(10);
    }
    let digits = bytes.len() - start;
    (digits > 0).then(|| (&text[..start], digits, value))
}

/// Valeur d'un texte décimal canonique, tel que l'écrit un `int` (sans zéro
/// de tête, sans `-0`).
pub(crate) fn canonical_int(text: &str) -> Option<i64> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let canonical = !digits.is_empty() && digits.len() <= 19 && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits.len() == 1 || !digits.starts_with('0')) && !(text.starts_with('-') && digits == "0");
    if canonical { text.parse().ok() } else { None }
}

/// Les 32 bits hauts décalent les valeurs négatives et lointaines.
pub(crate) fn hash_int(value: i64) -> u32 {
    let x = value as u64;
    (x as u32).wrapping_add(((x >> 32) as u32).wrapping_mul(0x9E37_79B9))
}

/// 32 bits hauts du produit par la constante dorée : tous les bits de `h`
/// y contribuent (les 32 bits bas de foldhash suivaient de trop près le
/// numéro de bloc pour certaines graines, et les blocs s'amassaient).
fn fold32(h: u64) -> u32 {
    (h.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as u32
}

pub(crate) fn hash_text(seed: &RandomState, text: &str) -> u32 {
    let mut hasher = seed.build_hasher();
    let Some((prefix, digits, value)) = numeric_suffix(text) else {
        // Texte d'un `int` de 19 chiffres (au-delà de `numeric_suffix`) ;
        // tout texte d'`int` finit par un chiffre.
        if let Some(value) = text.ends_with(|c: char| c.is_ascii_digit()).then(|| canonical_int(text)).flatten() {
            return hash_int(value);
        }
        hasher.write(text.as_bytes());
        return fold32(hasher.finish());
    };
    let canonical = digits == 1 || !text[prefix.len()..].starts_with('0');
    if canonical && (prefix.is_empty() || prefix == "-" && value != 0) {
        return hash_int(if prefix.is_empty() { value as i64 } else { (value as i64).wrapping_neg() });
    }
    hasher.write(prefix.as_bytes());
    // Le nombre de chiffres distingue `"k7"` de `"k007"`.
    hasher.write_u64((value / SUFFIX_BLOCK) ^ ((digits as u64) << 58));
    fold32(hasher.finish()).wrapping_add((value % SUFFIX_BLOCK) as u32)
}
