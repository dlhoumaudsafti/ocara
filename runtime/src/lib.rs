// ─────────────────────────────────────────────────────────────────────────────
// ocara_runtime — bibliothèque runtime d'Ocara v1.0
//
// Toutes les fonctions sont exportées avec la convention C (`extern "C"`,
// `#[unsafe(no_mangle)]`) pour être liées aux binaires produits par le compilateur.
//
// Représentation des valeurs :
//   - string  → i64 pointeur vers bytes UTF-8 null-terminated (heap ou .rodata)
//   - int     → i64 valeur directe
//   - float   → f64 valeur directe
//   - bool    → i64  (0 = false, 1 = true)
//   - array   → i64 pointeur vers OcaraArray (heap)
//   - map     → i64 pointeur vers OcaraMap   (heap)
//
// Distinction pointeur / entier :
//   Sur Linux/macOS, le noyau réserve les adresses < 0x10000 (64 Ko).
//   Toute adresse valide est donc >= 0x10000.
//   Les entiers pratiques dans Ocara sont < 0x10000 → pas d'ambiguïté.
//   Limitation : les entiers >= 65536 passés à `write()` seront traités
//   comme des pointeurs. Pour ces cas, utiliser Convert::intToStr() d'abord.
// ─────────────────────────────────────────────────────────────────────────────

#![allow(clippy::missing_safety_doc)]

use std::alloc::{alloc_zeroed, dealloc, Layout};
use crate::containers::{with_array, with_map, OcaraArray, OcaraMap};
use crate::map_table::{Key, KeyRef};
use crate::typecheck::{TAG_STRING_OWNED, TAG_ARRAY, TAG_MAP, TAG_FUNCTION,
    __is_function, __is_object, __is_map, __is_array, __is_string};
use std::io::{self, BufRead};
use std::process::Command;
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// I/O bas niveau — évite la récursion avec libc::write
//
// Notre symbole `write` shadowe le write(2) POSIX. Si l'on utilisait
// print!/println! (qui appellent libc::write(1,...) en interne), on
// entrerait en récursion infinie → stack overflow.
// On contourne en appelant directement le syscall SYS_write.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub(crate) fn write_stdout_raw(bytes: &[u8]) {
    // Appelle directement le syscall SYS_write pour éviter de shadower le `write(fd,buf,n)` POSIX
    // dont Rust's std::fs::write a besoin en interne.
    if bytes.is_empty() { return; }
    unsafe {
        core::arch::asm!(
            "syscall",
            inout("rax") 1isize => _,   // SYS_write → retour ignoré
            in("rdi") 1usize,           // fd = STDOUT_FILENO
            in("rsi") bytes.as_ptr(),
            in("rdx") bytes.len(),
            out("rcx") _,
            out("r11") _,
            options(nostack),
        );
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn write_stderr_raw(bytes: &[u8]) {
    // Même implémentation que write_stdout_raw, mais avec fd = 2 (STDERR_FILENO)
    // NOTE : on pourrait optimiser en combinant les deux fonctions et en passant le fd en paramètre,
    if bytes.is_empty() { return; }
    unsafe {
        core::arch::asm!(
            "syscall",
            inout("rax") 1isize => _,   // SYS_write → retour ignoré
            in("rdi") 2usize,           // fd = STDERR_FILENO
            in("rsi") bytes.as_ptr(),
            in("rdx") bytes.len(),
            out("rcx") _,
            out("r11") _,
            options(nostack),
        );
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn write_stdout_raw(bytes: &[u8]) {
    // Fallback : utilise println uniquement sur les plateformes non-Linux
    // où le conflit de symbole n'existe pas (macOS link dynamique par défaut).
    use std::io::Write as _;
    let _ = io::stdout().write_all(bytes);
    let _ = io::stdout().flush();
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn write_stderr_raw(bytes: &[u8]) {
    // Fallback : utilise eprintln uniquement sur les plateformes non-Linux
    // où le conflit de symbole n'existe pas (macOS link dynamique par défaut).
    use std::io::Write as _;
    let _ = io::stderr().write_all(bytes);
    let _ = io::stderr().flush();
}

fn ocara_print(s: &str) {
    write_stdout_raw(s.as_bytes());
}

fn ocara_println(s: &str) {
    write_stdout_raw(s.as_bytes());
    write_stdout_raw(b"\n");
}

// ─────────────────────────────────────────────────────────────────────────────
pub mod httprequest;
pub mod thread;
pub mod mutex;
pub mod httpserver;
mod coverage;
mod stack_guard;
mod httpserver_wire;
pub mod httpsession;
pub mod civil;
pub mod datetime;
pub mod date;
pub mod time;
pub mod typecheck;
pub mod htmlcomponent;
pub mod file;
pub mod directory;
pub mod exception;
pub mod sqlite;
pub mod mysql;
pub mod dotenv;
pub mod yaml;
pub mod rc;
pub mod task;
pub mod mixed_check;
pub mod str_append;
mod alloc_cache;
pub mod profile;
pub mod leak;
pub mod global_const;
pub mod blocking;
pub(crate) mod unittest_runtime;
pub(crate) mod containers;
pub(crate) mod map_table;
pub(crate) mod map_hash;
pub(crate) mod json_codec;
pub(crate) mod rc_concurrent;
// Tauri vit dans le crate séparé runtime_tauri (voir sa doc) : pas de `mod tauri`
// ici, pour que ce code (et sa dépendance GTK/WebKit) n'existe dans le binaire
// final QUE pour les programmes qui importent réellement ocara.Tauri.

// Helpers mémoire internes
// ─────────────────────────────────────────────────────────────────────────────

/// Alloue une chaîne null-terminated sur le heap et retourne son adresse.
/// Alignement 8 pour garantir que les 3 bits bas sont 0 (invariant boxing).
/// Taguée `TAG_STRING_OWNED` (pas `TAG_STRING`) : c'est ce qui distingue une
/// string réellement possédée (libérable) d'un littéral `.rodata` — voir la
/// doc de `TAG_STRING_OWNED` dans `typecheck.rs`. `alloc_str` est le SEUL
/// point de création d'une string dynamique dans tout le runtime (170+
/// appels à travers `runtime/src/*.rs`) : ce tag couvre donc uniformément
/// toute string hors littéral, sans exception à traiter ailleurs.
///
/// Layout : `[len: i64 @ raw][tag: i64 @ raw+8][données... @ raw+16][NUL]`.
/// Le tag reste à l'offset habituel `val - 8` (`read_tag`, `typecheck.rs`,
/// inchangé) ; `len` (la vraie longueur en octets, en plus du tag) est une
/// case supplémentaire AVANT le tag, invisible de tout le reste du runtime —
/// seule `free_str` la lit. Corrige un vrai risque de heap corruption :
/// `free_str` retrouvait auparavant la longueur en cherchant le premier
/// octet NUL depuis `val`, alors qu'une string Ocara PEUT légitimement
/// contenir un NUL en son milieu (`"a\0b"` : `\0` est un échappement de
/// chaîne valide, voir `src/parsing/lexer.d/scanner.d/readers.rs`) — un tel
/// octet interne aurait fait recalculer une longueur plus COURTE que
/// l'allocation réelle, donc un `Layout` faux passé à `dealloc` (voir
/// docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md). La libération
/// d'un objet (`rc::free_object`, runtime/src/rc.rs) n'a pas ce problème :
/// sa taille vient du masque de classe rangé dans l'en-tête de l'instance
/// (`src/lower/builder.d/rc_layout.rs`), le même qu'à l'allocation.
/// `pub` (pas `pub(crate)`) : utilisé depuis le crate séparé runtime_tauri.
pub unsafe fn alloc_str(s: &str) -> i64 {
    let bytes = s.as_bytes();
    unsafe {
        let val = rc::alloc_block(bytes.len() + 1, bytes.len() as i64, TAG_STRING_OWNED, false);
        let data = val as *mut u8;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        *data.add(bytes.len()) = 0u8;
        val
    }
}

/// Chaîne possédée de `len` octets remplis par `fill` (pas de copie depuis
/// une `String` intermédiaire). `fill` doit écrire de l'UTF-8 valide.
pub(crate) unsafe fn alloc_str_with(len: usize, fill: impl FnOnce(&mut [u8])) -> i64 {
    unsafe {
        let val = rc::alloc_block(len + 1, len as i64, TAG_STRING_OWNED, false);
        let data = std::slice::from_raw_parts_mut(val as *mut u8, len + 1);
        fill(&mut data[..len]);
        data[len] = 0;
        val
    }
}

/// Les cent paires de chiffres `"00"` à `"99"`.
const DIGIT_PAIRS: [u8; 200] = {
    let mut pairs = [0; 200];
    let mut i = 0;
    while i < 100 {
        pairs[2 * i] = b'0' + (i / 10) as u8;
        pairs[2 * i + 1] = b'0' + (i % 10) as u8;
        i += 1;
    }
    pairs
};

/// Texte décimal d'un entier, sans allocation ; deux chiffres par division.
pub(crate) fn int_text(n: i64, buf: &mut [u8; 20]) -> &str {
    let mut i = buf.len();
    let mut m = n.unsigned_abs();
    while m >= 10 {
        let pair = (m % 100) as usize * 2;
        m /= 100;
        i -= 2;
        buf[i..i + 2].copy_from_slice(&DIGIT_PAIRS[pair..pair + 2]);
    }
    // Chiffre de tête restant, ou zéro seul.
    if m > 0 || i == buf.len() {
        i -= 1;
        buf[i] = b'0' + m as u8;
    }
    if n < 0 {
        i -= 1;
        buf[i] = b'-';
    }
    // Chiffres et signe ASCII : de l'UTF-8 valide.
    unsafe { std::str::from_utf8_unchecked(&buf[i..]) }
}

/// Libère une chaîne allouée par `alloc_str` — exact inverse (même layout :
/// 8 octets de longueur + 8 octets de tag + données + NUL, alignement 8).
/// PAS un ramasse-miettes général : à utiliser uniquement sur des temporaires
/// dont la durée de vie est connue et courte (ex. valeurs FFI internes à un
/// appel), jamais sur une valeur Ocara ordinaire potentiellement encore
/// référencée ailleurs — ce runtime ne fait aucun suivi de références,
/// appeler ceci sur un pointeur encore utilisé ailleurs est un use-after-free.
/// `pub` (pas `pub(crate)`) : utilisé depuis le crate séparé runtime_tauri.
#[inline]
pub unsafe fn free_str(val: i64) {
    if val == 0 {
        return;
    }
    unsafe {
        // Longueur réelle stockée à l'allocation (voir alloc_str) — plus
        // fiable qu'un recalcul par recherche du premier octet NUL, qui
        // sous-estimerait la taille si la string contient un NUL interne.
        rc::free_block(val, str_append::str_payload(val));
    }
}

/// Lit un pointeur i64 comme `&str` — littéral (`TAG_STRING`) ou possédée
/// (`TAG_STRING_OWNED`), les deux partagent désormais le MÊME layout de
/// header (`[len: i64][tag: i64][données...][NUL]`, voir `alloc_str`/
/// `emit_strings` dans le codegen) : la vraie longueur est lue directement
/// à `val - 16`, plutôt que de tronquer au premier octet NUL (`CStr::
/// from_ptr`, l'ancien comportement) — une string Ocara PEUT légitimement
/// contenir un NUL interne (`"a\0b"`, un échappement valide), qui restait
/// jusqu'ici affiché/comparé tronqué partout dans le langage (voir
/// docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md).
/// `pub` (pas `pub(crate)`) : utilisé depuis le crate séparé runtime_tauri.
pub unsafe fn ptr_to_str<'a>(val: i64) -> &'a str {
    if val == 0 {
        return "";
    }
    unsafe {
        let len = str_append::str_len(val);
        let bytes = std::slice::from_raw_parts(val as *const u8, len);
        // Toute chaîne Ocara est écrite depuis un `&str` (`alloc_str`,
        // `alloc_str_with`, `__str_append`, littéraux du codegen) : déjà de
        // l'UTF-8 valide, la revalider coûtait une passe à chaque lecture.
        debug_assert!(std::str::from_utf8(bytes).is_ok());
        std::str::from_utf8_unchecked(bytes)
    }
}

// ─── Tagged-pointer boxing pour les valeurs `any` ───────────────────────────
// Les pointeurs heap sont alignés sur 8 octets → les 3 bits bas sont 0.
// On encode le type dans les 2 bits bas :
//   bits 1:0 = 00  → string ou objet normal (is_ptr)
//   bits 1:0 = 01  → float boxé  (__box_float)
//   bits 1:0 = 10  → bool boxé   (__box_bool)
//   bits 1:0 = 11  → int boxé   (__box_int_for_mixed) — voir plus bas
// Un `int` de `mixed` de 0 à 2⁵⁹ est rangé en ligne, sans cellule :
// `INLINE_INT_TAG | n << 3` (bits 2:0 = 100, bit 62 posé, bit 63 nul) —
// jamais une adresse (≥ 2⁶²), ni un entier brut (négatif ou < 0x10000).
// Un `bool` de `mixed` l'est toujours : `INLINE_BOOL_TAG | b << 3` (bits
// 2:0 = 010, lu par `is_bool_box` comme un bool boxé). Toute valeur comptée
// (pointeur, cellule) est sous `POINTER_LIMIT`.

/// Retourne true si val est un pointeur string/objet (bits bas == 000).
#[inline]
fn is_ptr(val: i64) -> bool {
    val >= 0x10000 && (val & 7) == 0
}

/// Au-dessus de toute adresse d'un processus (48 bits sur aarch64, 57 au
/// plus sur x86-64), sous les valeurs en ligne (≥ 2⁶²).
pub(crate) const POINTER_LIMIT: i64 = 1 << 56;
const INLINE_INT_TAG: u64 = 0x4000_0000_0000_0004;
const INLINE_BOOL_TAG: u64 = 0x4000_0000_0000_0002;
const INLINE_INT_MASK: u64 = 0xC000_0000_0000_0007;
const INLINE_INT_LIMIT: i64 = 1 << 59;

/// `int` de `mixed` rangé en ligne (voir `box_int_if_needed`).
#[inline]
pub(crate) fn is_inline_int(val: i64) -> bool {
    val as u64 & INLINE_INT_MASK == INLINE_INT_TAG
}

#[inline]
pub(crate) fn is_float_box(val: i64) -> bool {
    val >= 0x10000 && (val & 3) == 1
}

#[inline]
pub(crate) fn is_bool_box(val: i64) -> bool {
    val >= 0x10000 && (val & 3) == 2
}

/// Voir `box_int_if_needed` : un `int` logé dans un `mixed` est boxé s'il est
/// assez grand pour être confondu avec un pointeur heap (au-delà de ce seuil,
/// un pointeur réel ET un entier ordinaire sont indiscernables sans boxing —
/// voir docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md), OU s'il vaut
/// `0` (sinon indiscernable de `null`, voir la doc de `box_int_if_needed`) —
/// tout autre petit entier reste brut, jamais alloué.
#[inline]
pub(crate) fn is_int_box(val: i64) -> bool {
    is_inline_int(val) || (val >= 0x10000 && (val & 3) == 3)
}

#[inline]
pub(crate) unsafe fn unbox_float(val: i64) -> f64 {
    unsafe { *((val & !3) as *const f64) }
}

#[inline]
pub(crate) unsafe fn unbox_bool(val: i64) -> bool {
    if val as u64 & INLINE_INT_MASK == INLINE_BOOL_TAG {
        return val >> 3 & 1 != 0;
    }
    unsafe { *((val & !3) as *const i64) != 0 }
}

#[inline]
pub(crate) unsafe fn unbox_int(val: i64) -> i64 {
    if is_inline_int(val) {
        return ((val as u64 ^ INLINE_INT_TAG) >> 3) as i64;
    }
    unsafe { *((val & !3) as *const i64) }
}

/// Boxe `n` s'il est assez grand pour être confondu avec un pointeur heap
/// valide par `read_tag`/`get_value_type` (`val >= 0x10000`, le même seuil
/// que `PTR_THRESHOLD` ailleurs dans le runtime), OU s'il vaut exactement
/// `0` — un petit entier NON NUL reste brut (comme avant ce correctif), pas
/// de coût d'allocation pour le cas de loin le plus fréquent. Un entier
/// NÉGATIF (non nul) n'est jamais ambigu avec un pointeur (toujours <
/// 0x10000 en comparaison signée) et reste donc toujours brut, quelle que
/// soit sa magnitude.
///
/// Le cas `n == 0` est spécial : `null` est TOUJOURS représenté par le bit
/// pattern `0` (voir `Expr::Literal(Literal::Null, _)` dans le lowering) —
/// un entier brut valant `0` logé dans un `mixed` serait donc structurellement
/// indiscernable de `null`, les deux valant `0`. Boxer spécifiquement `0`
/// élimine cette ambiguïté : toute décision consultant `get_value_type`/
/// `is_int_box`/`__is_null`/`__is_int` (déjà correctes pour un entier boxé,
/// écrites par anticipation de ce cas) traite alors correctement un entier
/// `0` authentique comme "un entier valant 0", jamais comme `null` — confirmé
/// faux par reproduction : `Array::get`/`Map::get` sur un élément `0`,
/// `UnitTest::assertEquals(0, ...)`, affichaient/traitaient la valeur comme
/// `null` (voir docs/roadmap.d/langage-array-get-display-bug.md).
///
/// Contrairement à `float`/`bool` (toujours boxés dans un `mixed`, sans
/// condition), cette fonction est LE point d'entrée unique qui décide, au
/// runtime, si un `int` donné a besoin d'être boxé — appelée à chaque
/// endroit qui logeait auparavant un `int` brut dans un `mixed` (affectation,
/// argument, littéral `array`/`map`, résultat arithmétique dynamique...).
///
/// Un `int` à boxer jusqu'à 2⁵⁹ (`0` compris, cas de loin le plus courant)
/// est rangé en ligne, sans allocation ; seul un plus grand passe par une
/// cellule (`ptr | 3`) — chaque opération `mixed` allouait sa cellule.
#[inline]
pub(crate) fn box_int_if_needed(n: i64) -> i64 {
    if n != 0 && n < 0x10000 {
        return n;
    }
    if n < INLINE_INT_LIMIT {
        return (INLINE_INT_TAG | (n as u64) << 3) as i64;
    }
    int_cell(n)
}

#[cold]
#[inline(never)]
fn int_cell(n: i64) -> i64 {
    unsafe { rc::alloc_box(n, 3) }
}

/// Convertit n'importe quelle valeur i64 (int, float boxé, bool boxé, string ptr) en String.
fn val_to_string(val: i64) -> String {
    // null (pointeur nul = 0) → affiche "null"
    if val == 0 {
        return "null".to_string();
    }
    if is_float_box(val) {
        float_text(unsafe { unbox_float(val) })
    } else if is_bool_box(val) {
        unsafe { if unbox_bool(val) { "true".to_string() } else { "false".to_string() } }
    } else if is_int_box(val) {
        unsafe { unbox_int(val).to_string() }
    } else if is_ptr(val) {
        ptr_to_display(val, 0)
    } else {
        val.to_string()
    }
}

/// Valeur heap affichable : chaîne telle quelle, tableau `[a, b]`, map
/// `{k: v}` (éléments selon leur genre brut, voir `fmt_elem`), objet ou
/// fonction par leur genre. Un conteneur était lu comme une chaîne (octets
/// bruts affichés par `IO::writeln(array)`). `depth` borne les cycles.
fn ptr_to_display(val: i64, depth: usize) -> String {
    if depth > 16 {
        return "…".to_string();
    }
    match get_value_type(val) {
        5 => {
            let kind = rc::raw_kind(val);
            let items: Vec<String> = unsafe { array_data(val) }.iter().map(|&v| nested_elem(kind, v, depth)).collect();
            format!("[{}]", items.join(", "))
        }
        6 => {
            let kind = rc::raw_kind(val);
            let items: Vec<String> = unsafe { map_entries(val) }.iter().map(|(k, v)| format!("{}: {}", k, nested_elem(kind, *v, depth))).collect();
            format!("{{{}}}", items.join(", "))
        }
        7 => "object".to_string(),
        8 => "function".to_string(),
        _ => unsafe { ptr_to_str(val).to_string() },
    }
}

/// Élément de conteneur pour `ptr_to_display` (conteneurs imbriqués compris).
fn nested_elem(kind: i64, v: i64, depth: usize) -> String {
    if kind == 0 && is_ptr(v) {
        return ptr_to_display(v, depth + 1);
    }
    fmt_elem(kind, v)
}

/// Formate un i64 en représentation affichable.
fn fmt_val(val: i64) -> String {
    val_to_string(val)
}

// ─────────────────────────────────────────────────────────────────────────────
// Structures heap
// ─────────────────────────────────────────────────────────────────────────────

fn new_array() -> i64 {
    new_array_with(Vec::new())
}

/// Tableau neuf contenant `data` (éléments déjà possédés par le tableau).
fn new_array_with(data: Vec<i64>) -> i64 {
    unsafe {
        let val = rc::alloc_block(std::mem::size_of::<OcaraArray>(), 0, TAG_ARRAY, false);
        std::ptr::write(val as *mut OcaraArray, OcaraArray::new(data));
        val
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_new() -> i64 {
    new_array()
}

/// Vrai pour un conteneur à éléments `int`/`float`/`bool` bruts (voir
/// `rc::FLAG_RAW`) : ses éléments ne sont jamais comptés.
pub(crate) fn is_raw_container(ptr: i64) -> bool {
    unsafe { rc::aux(ptr) & rc::FLAG_RAW != 0 }
}

/// Retient un élément qui entre dans le conteneur `ptr` (argument emprunté).
fn retain_elem(ptr: i64, val: i64) {
    if !is_raw_container(ptr) { unsafe { rc::retain(val) } }
}

/// Relâche un élément qui sort du conteneur `ptr`.
fn release_elem(ptr: i64, val: i64) {
    if !is_raw_container(ptr) { unsafe { rc::release(val) } }
}

/// Copie les éléments `vals` de `src` dans un nouveau tableau : chacun est
/// retenu, le drapeau « éléments bruts » est propagé.
fn array_sharing(src: i64, vals: Vec<i64>) -> i64 {
    let dst = new_array();
    if is_raw_container(src) {
        rc::copy_raw(src, dst);
    } else {
        for &v in &vals { unsafe { rc::retain(v) } }
    }
    unsafe { with_array(dst, |data| *data = vals) }
    dst
}

/// Insertion interne d'une valeur déjà possédée (fraîchement allouée par le
/// runtime) : transférée au tableau, sans retenue.
pub(crate) fn array_push_owned(ptr: i64, val: i64) {
    if ptr == 0 { return; }
    unsafe { with_array(ptr, |data| data.push(val)) }
}

/// `__array_push` d'une valeur dont l'appelant cède la référence (élément
/// possédé d'un littéral `[…]`) : sans retenue ni relâchement.
#[unsafe(no_mangle)]
pub extern "C" fn __array_push_owned(ptr: i64, val: i64) {
    array_push_owned(ptr, val);
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_push(ptr: i64, val: i64) {
    if ptr == 0 { return; }
    retain_elem(ptr, val);
    array_push_owned(ptr, val);
}

pub(crate) fn new_map() -> i64 {
    unsafe {
        let val = rc::alloc_block(std::mem::size_of::<OcaraMap>(), 0, TAG_MAP, false);
        std::ptr::write(val as *mut OcaraMap, OcaraMap::new());
        val
    }
}

pub(crate) unsafe fn array_data(ptr: i64) -> Vec<i64> {
    unsafe { with_array(ptr, |data| data.clone()) }
}

pub(crate) unsafe fn map_values(ptr: i64) -> Vec<i64> {
    unsafe { with_map(ptr, |data| data.values().collect()) }
}

pub(crate) unsafe fn drop_array_block(ptr: i64) {
    unsafe {
        std::ptr::drop_in_place(ptr as *mut OcaraArray);
        rc::free_block(ptr, std::mem::size_of::<OcaraArray>());
    }
}

pub(crate) unsafe fn drop_map_block(ptr: i64) {
    unsafe {
        std::ptr::drop_in_place(ptr as *mut OcaraMap);
        rc::free_block(ptr, std::mem::size_of::<OcaraMap>());
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// I/O de base — write / read
// ─────────────────────────────────────────────────────────────────────────────

/// Fonction interne (pas exportée en C) — écrit une valeur sur stdout.
/// NOTE : pas de #[unsafe(no_mangle)] pour éviter de shadower le `write(fd,buf,n)` POSIX
///        dont Rust's std::fs::write a besoin en interne.
#[allow(dead_code)]
fn write(val: i64) {
    ocara_print(&fmt_val(val));
}

#[unsafe(no_mangle)]
pub extern "C" fn __str_concat(a: i64, b: i64) -> i64 {
    let sa = val_to_string(a);
    let sb = val_to_string(b);
    unsafe { alloc_str(&(sa + &sb)) }
}

/// Convertit n'importe quelle valeur I64 en string (pour les templates).
/// Retourne un argument emprunté tel quel : retenu, comme toute valeur
/// retournée par un builtin (+1).
fn __rc_passthrough(val: i64) -> i64 {
    unsafe { rc::retain(val) }
    val
}

#[unsafe(no_mangle)]
pub extern "C" fn __val_to_str(val: i64) -> i64 {
    if is_float_box(val) || is_bool_box(val) || is_int_box(val) {
        unsafe { alloc_str(&val_to_string(val)) }
    } else if is_ptr(val) {
        __rc_passthrough(val)
    } else {
        unsafe { alloc_str(&val.to_string()) }
    }
}

/// Boxe un float (bits i64) dans une cellule heap ; retourne `ptr | 1`.
#[unsafe(no_mangle)]
pub extern "C" fn __box_float(bits: i64) -> i64 {
    unsafe { rc::alloc_box(bits, 1) }
}

/// Bool d'un `mixed`, rangé en ligne (`INLINE_BOOL_TAG | b << 3`) : une
/// cellule allouée par booléen auparavant.
#[unsafe(no_mangle)]
pub extern "C" fn __box_bool(b: i64) -> i64 {
    (INLINE_BOOL_TAG | u64::from(b != 0) << 3) as i64
}

/// Point d'entrée appelé depuis le lowering partout où un `int` (connu
/// statiquement, ou déjà en transit dans une expression "mixed") est logé
/// dans un `mixed` — voir `box_int_if_needed` (seuls les entiers assez grands
/// pour être ambigus avec un pointeur heap sont réellement boxés ; retourne
/// `ptr | 3`, sinon `n` inchangé).
#[unsafe(no_mangle)]
pub extern "C" fn __box_int_for_mixed(n: i64) -> i64 {
    box_int_if_needed(n)
}

/// Texte d'un `float` : toujours une partie décimale (`42.0`, comme
/// Python), pour qu'un flottant entier ne se confonde pas avec un `int`
/// (`42`) ; `inf`, `-inf` et `NaN` inchangés.
pub(crate) fn float_text(f: f64) -> String {
    let mut text = f.to_string();
    if f.is_finite() && !text.contains('.') {
        text.push_str(".0");
    }
    text
}

#[unsafe(no_mangle)]
pub extern "C" fn __str_from_float(f: f64) -> i64 {
    unsafe { alloc_str(&float_text(f)) }
}

/// Conversion entier -> flottant pour le widening implicite des comparaisons
/// typées (`equal`/`smaller`/`greater`/...) entre `int` et `float` : jamais un
/// bitcast (qui reinterpreterait les bits de l'entier comme un float
/// n'importe-quoi), une vraie conversion numerique. Interne uniquement — pas
/// lie a l'import `ocara.Convert` (voir Convert_intToFloat, la version
/// publique identique mais gatee par import), car requis meme dans un
/// programme qui n'importe pas `Convert`.
#[unsafe(no_mangle)]
pub extern "C" fn __int_to_float(n: i64) -> f64 {
    n as f64
}

#[unsafe(no_mangle)]
pub extern "C" fn __str_from_bool(b: i64) -> i64 {
    unsafe { alloc_str(if b != 0 { "true" } else { "false" }) }
}

/// Convertit un entier i64 en string — sans heuristique pointeur.
/// À utiliser dans les templates `${expr}` quand le type est I64.
#[unsafe(no_mangle)]
pub extern "C" fn __str_from_int(n: i64) -> i64 {
    unsafe { alloc_str(int_text(n, &mut [0; 20])) }
}

/// Convertit un tableau en string au format `[a, b, c]`.
#[unsafe(no_mangle)]
pub extern "C" fn __array_to_str(ptr: i64) -> i64 {
    if ptr == 0 {
        return unsafe { alloc_str("[]") };
    }
    // Tableaux imbriqués compris (`${array<array<int>>}`).
    unsafe { alloc_str(&ptr_to_display(ptr, 0)) }
}

/// Élément d'un conteneur, selon son genre brut (`rc::raw_kind` : 1 int,
/// 2 float, 3 bool, 0 valeur auto-décrite). Un entier ou un flottant brut lu
/// par `fmt_val` était pris pour un pointeur (SIGSEGV sur `${array<float>}`).
pub(crate) fn fmt_elem(kind: i64, v: i64) -> String {
    match kind {
        1 => v.to_string(),
        2 => float_text(f64::from_bits(v as u64)),
        3 => (if v != 0 { "true" } else { "false" }).to_string(),
        _ => fmt_val(v),
    }
}

/// Retourne le nom du système d'exploitation cible.
///
/// `target_os = "android"` est une valeur DISTINCTE de `"linux"` pour rustc
/// (même si le triple contient "linux", ex. `aarch64-linux-android`) —
/// vérifié (`rustc --print cfg --target x86_64-linux-android` ne donne QUE
/// `target_os="android"`, jamais `"linux"` en plus). Avant cette branche,
/// un programme Ocara cross-compilé pour Android obtenait "unknown" ici,
/// découvert en testant `examples/advanced/mini_project/main_android.oc`
/// dans un émulateur réel (voir docs/roadmap.d/packaging-android-webview-hybrid.md) —
/// le chemin absolu du fichier SQLite doit différer sur Android (bac à
/// sable de l'app, voir `configs/Database.oc`), ce qui a rendu ce manque
/// concret plutôt que théorique.
#[unsafe(no_mangle)]
pub extern "C" fn __system_os() -> i64 {
    let os = if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };
    unsafe { alloc_str(os) }
}

/// Retourne l'architecture cible.
#[unsafe(no_mangle)]
pub extern "C" fn __system_arch() -> i64 {
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "unknown"
    };
    unsafe { alloc_str(arch) }
}

#[unsafe(no_mangle)]
pub extern "C" fn write_int(n: i64) {
    ocara_print(&n.to_string());
}

#[unsafe(no_mangle)]
pub extern "C" fn write_float(f: f64) {
    ocara_print(&float_text(f));
}

#[unsafe(no_mangle)]
pub extern "C" fn write_bool(b: i64) {
    ocara_print(if b != 0 { "true" } else { "false" });
}

/// Fonction interne (pas exportée en C) — lit une ligne sur stdin.
/// NOTE : pas de #[unsafe(no_mangle)] pour éviter de shadower le `read(fd,buf,n)` POSIX
///        dont Rust's io::stdin() a besoin en interne.
/// Lève une IOException en cas d'erreur de lecture.
fn read() -> i64 {
    let mut line = String::new();
    let outcome = { let _parked = rc::park(); io::stdin().lock().read_line(&mut line) };
    match outcome {
        // Fin de l'entrée (stdin fermé) : `IOException` 101 (IO.md), au lieu
        // d'une ligne vide indiscernable d'une vraie ligne vide.
        Ok(0) => unsafe {
            exception::throw_io_exception("Failed to read from stdin: end of input", ERR_IO_READ, "IO");
        },
        Ok(_) => {
            if line.ends_with('\n') { line.pop(); }
            if line.ends_with('\r') { line.pop(); }
            unsafe { alloc_str(&line) }
        }
        Err(e) => unsafe {
            exception::throw_io_exception(
                &format!("Failed to read from stdin: {}", e),
                ERR_IO_READ,
                "IO"
            );
        }
    }
}

/// Alias exporté pour les programmes Ocara qui appellent `read()` directement
#[unsafe(no_mangle)]
pub extern "C" fn ocara_read() -> i64 {
    read()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tableaux internes (__array_*, __range)
// ─────────────────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn __range(lo: i64, hi: i64) -> i64 {
    let ptr = rc::__rc_mark_raw(new_array());
    unsafe { with_array(ptr, |data| data.extend(lo..hi)) }
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_len(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_array(ptr, |data| data.len() as i64) }
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_get(ptr: i64, idx: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_array(ptr, |data| data.get(idx as usize).copied().unwrap_or(0)) }
}

/// Élément `idx` sorti du tableau, case remise à vide : sa référence passe à
/// l'appelant (clé relevée par `Map_keys`, reprise par la variable de
/// `for k has v in m`).
#[unsafe(no_mangle)]
pub extern "C" fn __array_take(ptr: i64, idx: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_array(ptr, |data| data.get_mut(idx as usize).map_or(0, std::mem::take)) }
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_set(ptr: i64, idx: i64, val: i64) {
    if ptr == 0 { return; }
    let i = idx as usize;
    retain_elem(ptr, val);
    let old = unsafe {
        with_array(ptr, |data| {
            if data.len() <= i {
                data.resize(i + 1, 0);
            }
            std::mem::replace(&mut data[i], val)
        })
    };
    release_elem(ptr, old);
}

// ─────────────────────────────────────────────────────────────────────────────
// Maps internes (__map_*)
// ─────────────────────────────────────────────────────────────────────────────

/// Convertit une clé i64 en String pour la comparer par valeur.
/// Si c'est un pointeur vers une string, on lit son contenu.
/// Sinon, on convertit l'entier en string décimale.
unsafe fn key_to_string(key: i64) -> String {
    // Clé non-chaîne d'une `map<int|float|bool, V>` : boxée par le
    // compilateur, convertie en texte ici (lue comme une chaîne sinon).
    if is_float_box(key) || is_bool_box(key) || is_int_box(key) {
        val_to_string(key)
    } else if is_ptr(key) {
        unsafe { ptr_to_str(key).to_string() }
    } else {
        key.to_string()
    }
}

/// Appelle `f` avec le texte de la clé, sans copie pour une clé chaîne.
unsafe fn with_key_text<R>(key: i64, f: impl FnOnce(&str) -> R) -> R {
    if is_ptr(key) && !is_float_box(key) && !is_bool_box(key) && !is_int_box(key) {
        f(unsafe { ptr_to_str(key) })
    } else {
        f(&unsafe { key_to_string(key) })
    }
}

/// Clé texte d'une map → valeur brute du type de clé déclaré `K` (`kind` :
/// 1 = int, 2 = float, 3 = bool ; sinon la chaîne, retenue).
fn key_from_text(text: &str, kind: i64) -> i64 {
    match kind {
        1 => text.parse::<i64>().unwrap_or(0),
        2 => text.parse::<f64>().unwrap_or(0.0).to_bits() as i64,
        3 => i64::from(text == "true"),
        _ => unsafe { alloc_str(text) },
    }
}

/// Clé (chaîne) d'une `map<K, V>` relue au type `K` — variable de clé de
/// `for k has v in m` (voir docs/roadmap.d/langage-map-loop-value-type.md).
#[unsafe(no_mangle)]
pub extern "C" fn __map_key_as(key: i64, kind: i64) -> i64 {
    if key == 0 { return 0; }
    key_from_text(unsafe { ptr_to_str(key) }, kind)
}

#[unsafe(no_mangle)]
pub extern "C" fn __map_new() -> i64 {
    new_map()
}

/// Insertion interne d'une clé ET d'une valeur allouées par le runtime pour
/// l'occasion : la valeur est transférée à la map (sans retenue), la clé,
/// dont la map garde une copie, est relâchée — sans quoi chaque ligne
/// SQLite/MySQL, chaque objet JSON/YAML, chaque en-tête… fuyait.
pub fn map_set_owned_key(ptr: i64, key: i64, val: i64) {
    map_put(ptr, key, val);
    unsafe { rc::release(key) }
}

/// Range `val` sous `key` sans le retenir ; l'ancienne valeur est relâchée.
/// Une clé chaîne est retenue et reprise telle quelle par la map.
fn map_put(ptr: i64, key: i64, val: i64) {
    if ptr == 0 { return; }
    let owned = (is_ptr(key) && typecheck::__is_string(key) != 0).then(|| {
        unsafe { rc::retain(key) };
        key
    });
    let old = unsafe { with_key_text(key, |text| with_map(ptr, |data| data.insert(Key::Text(text), val, owned))) };
    if let Some(old) = old {
        release_elem(ptr, old);
    }
}

/// `__map_set` d'une valeur dont l'appelant cède la référence (élément
/// possédé d'un littéral `{…}`) : sans retenue ni relâchement.
#[unsafe(no_mangle)]
pub extern "C" fn __map_set_owned(ptr: i64, key: i64, val: i64) {
    map_put(ptr, key, val);
}

#[unsafe(no_mangle)]
pub extern "C" fn __map_set(ptr: i64, key: i64, val: i64) {
    if ptr == 0 { return; }
    retain_elem(ptr, val);
    map_put(ptr, key, val);
}

/// Range `val` sous `key` : une chaîne n'est allouée pour la clé qu'à
/// l'insertion d'une clé nouvelle dans une map à clés texte.
fn map_set_key(ptr: i64, key: Key, val: i64) {
    retain_elem(ptr, val);
    let old = unsafe { with_map(ptr, |data| data.insert(key, val, None)) };
    if let Some(old) = old {
        release_elem(ptr, old);
    }
}

/// Valeur (empruntée) sous `key` ; `0` si absente.
fn map_get_key(ptr: i64, key: Key) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_map(ptr, |data| data.get(key).unwrap_or(0)) }
}

/// `m[k] = v` pour une clé `int` brute.
#[unsafe(no_mangle)]
pub extern "C" fn __map_set_ikey(ptr: i64, key: i64, val: i64) {
    if ptr == 0 { return; }
    map_set_key(ptr, Key::Int(key), val)
}

#[unsafe(no_mangle)]
pub extern "C" fn __map_get_ikey(ptr: i64, key: i64) -> i64 {
    map_get_key(ptr, Key::Int(key))
}

/// `m[`…${x}…`] = v` : clé gabarit (parties comme `__str_build`), formatée
/// dans un tampon réutilisé.
#[unsafe(no_mangle)]
pub extern "C" fn __map_set_tpl(ptr: i64, parts: *const i64, n: i64, val: i64) {
    if ptr == 0 { return; }
    str_append::with_template_text(parts, n, |text| map_set_key(ptr, Key::Text(text), val))
}

/// `m[`…${x}…`]` : lecture sans allouer la chaîne de la clé.
#[unsafe(no_mangle)]
pub extern "C" fn __map_get_tpl(ptr: i64, parts: *const i64, n: i64) -> i64 {
    str_append::with_template_text(parts, n, |text| map_lookup(ptr, text))
}

/// Valeur (empruntée) sous la clé `key`, sans allouer de chaîne Ocara pour
/// la clé ; `0` si absente. `pub` : utilisé par runtime_sdl/runtime_tauri.
pub fn map_lookup(ptr: i64, key: &str) -> i64 {
    map_get_key(ptr, Key::Text(key))
}

#[unsafe(no_mangle)]
pub extern "C" fn __map_get(ptr: i64, key: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_key_text(key, |k| map_lookup(ptr, k)) }
}

/// Valeur de la clé `key` relevée au rang `index` par `Map_keys` (`for k has
/// v in m`) : la paire `index` si elle porte toujours cette clé, sans
/// hachage ; sinon recherche.
#[unsafe(no_mangle)]
pub extern "C" fn __map_value_at(ptr: i64, index: i64, key: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_key_text(key, |k| with_map(ptr, |data| data.value_at(index as usize, Key::Text(k)).unwrap_or(0))) }
}

/// `__map_value_at` pour une clé `int` brute.
#[unsafe(no_mangle)]
pub extern "C" fn __map_value_at_ikey(ptr: i64, index: i64, key: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_map(ptr, |data| data.value_at(index as usize, Key::Int(key)).unwrap_or(0)) }
}

/// Clé de rang `index`, empruntée — `for k has v in m` dont le corps ne peut
/// pas modifier la map : entier brut pour `int_key`, sinon chaîne.
#[unsafe(no_mangle)]
pub extern "C" fn __map_key_at(ptr: i64, index: i64, int_key: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_map(ptr, |data| data.key_at(index as usize, int_key != 0)) }
}

/// Valeur (empruntée) de rang `index`.
#[unsafe(no_mangle)]
pub extern "C" fn __map_value_index(ptr: i64, index: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_map(ptr, |data| data.value_index(index as usize)) }
}

/// Paires (clé retenue +1, valeur empruntée) d'une map, sans recopier le
/// texte des clés (voir `json_codec`).
pub(crate) unsafe fn map_shared_entries(ptr: i64) -> Vec<(i64, i64)> {
    if ptr == 0 { return Vec::new(); }
    unsafe { with_map(ptr, |data| data.iter().map(|(k, v)| (k.share(), v)).collect()) }
}

/// Copie les paires (clé, valeur `mixed` brute) d'une `map<string, mixed>` —
/// pour un appelant hors de ce fichier (ex. `sqlite::bind`) qui doit itérer
/// tout le contenu sans exposer `OcaraMap` lui-même.
pub(crate) unsafe fn map_entries(ptr: i64) -> Vec<(String, i64)> {
    if ptr == 0 { return Vec::new(); }
    unsafe { with_map(ptr, |data| data.iter().map(|(k, v)| (k.with_text(str::to_string), v)).collect()) }
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.IO
//
// Codes d'erreur IOException :
//   101 - READ  : Erreur de lecture depuis stdin
//   102 - WRITE : Erreur d'écriture sur stdout
// ─────────────────────────────────────────────────────────────────────────────

const ERR_IO_READ: i64 = 101;
#[allow(dead_code)]  // Réservé pour une future implémentation d'erreurs write
const ERR_IO_WRITE: i64 = 102;

#[unsafe(no_mangle)]
pub extern "C" fn IO_write(val: i64) {
    ocara_print(&fmt_val(val));
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_writeInt(n: i64) {
    ocara_print(&n.to_string());
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_writeFloat(f: f64) {
    ocara_print(&float_text(f));
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_writeBool(b: i64) {
    ocara_print(if b != 0 { "true" } else { "false" });
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_writeln(val: i64) {
    ocara_println(&fmt_val(val));
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_writelnInt(n: i64) {
    ocara_println(&n.to_string());
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_writelnFloat(f: f64) {
    ocara_println(&float_text(f));
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_writelnBool(b: i64) {
    ocara_println(if b != 0 { "true" } else { "false" });
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_read() -> i64 {
    read()
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readln() -> i64 {
    read()
}

/// Ligne lue copiée en `String` ; la chaîne Ocara intermédiaire est relâchée.
fn read_text() -> Option<String> {
    let s = read();
    if s == 0 { return None; }
    let text = unsafe { ptr_to_str(s).to_string() };
    unsafe { rc::release(s) }
    Some(text)
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readInt() -> i64 {
    read_text().map_or(0, |t| t.trim().parse::<i64>().unwrap_or(0))
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readFloat() -> f64 {
    read_text().map_or(0.0, |t| t.trim().parse::<f64>().unwrap_or(0.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readBool() -> i64 {
    let t = read_text().unwrap_or_default().trim().to_lowercase();
    if t == "true" || t == "1" { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readArray(sep: i64) -> i64 {
    let Some(src) = read_text() else { return new_array() };
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep).to_string() } } else { " ".to_string() };
    let ptr = new_array();
    for part in src.split(sep_s.as_str()) {
        array_push_owned(ptr, unsafe { alloc_str(part) });
    }
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readMap(sep: i64, kv: i64) -> i64 {
    let Some(src) = read_text() else { return new_map() };
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep).to_string() } } else { " ".to_string() };
    let kv_s  = if is_ptr(kv)  { unsafe { ptr_to_str(kv).to_string() }  } else { "=".to_string() };
    let ptr = new_map();
    for part in src.split(sep_s.as_str()) {
        if let Some(pos) = part.find(kv_s.as_str()) {
            let v = unsafe { alloc_str(&part[pos + kv_s.len()..]) };
            unsafe { map_set_owned_key(ptr, alloc_str(&part[..pos]), v); }
        }
    }
    ptr
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.String
// ─────────────────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn String_len(s: i64) -> i64 {
    if !is_ptr(s) { return 0; }
    let text = unsafe { ptr_to_str(s) };
    if text.is_ascii() { text.len() as i64 } else { text.chars().count() as i64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_upper(s: i64) -> i64 {
    if !is_ptr(s) { return __rc_passthrough(s); }
    let text = unsafe { ptr_to_str(s) };
    if text.is_ascii() {
        return unsafe { alloc_str_with(text.len(), |buf| { buf.copy_from_slice(text.as_bytes()); buf.make_ascii_uppercase() }) };
    }
    unsafe { alloc_str(&text.to_uppercase()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_lower(s: i64) -> i64 {
    if !is_ptr(s) { return __rc_passthrough(s); }
    let text = unsafe { ptr_to_str(s) };
    if text.is_ascii() {
        return unsafe { alloc_str_with(text.len(), |buf| { buf.copy_from_slice(text.as_bytes()); buf.make_ascii_lowercase() }) };
    }
    unsafe { alloc_str(&text.to_lowercase()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_capitalize(s: i64) -> i64 {
    if !is_ptr(s) { return __rc_passthrough(s); }
    let src = unsafe { ptr_to_str(s) };
    let mut chars = src.chars();
    let r = match chars.next() {
        None    => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    };
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_trim(s: i64) -> i64 {
    if !is_ptr(s) { return __rc_passthrough(s); }
    let r = unsafe { ptr_to_str(s) }.trim().to_string();
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_replace(s: i64, from: i64, to: i64) -> i64 {
    if !is_ptr(s) { return __rc_passthrough(s); }
    let src    = unsafe { ptr_to_str(s) };
    let from_s = if is_ptr(from) { unsafe { ptr_to_str(from) } } else { "" };
    let to_s   = if is_ptr(to)   { unsafe { ptr_to_str(to) } }   else { "" };
    // `from` vide : chaîne inchangée (sinon `to` serait inséré entre chaque caractère).
    if from_s.is_empty() {
        return unsafe { alloc_str(src) };
    }
    // Une seule passe de recherche, dans un tampon réutilisé.
    str_append::with_buffer(|text| {
        let mut last = 0;
        for (start, _) in src.match_indices(from_s) {
            text.push_str(&src[last..start]);
            text.push_str(to_s);
            last = start + from_s.len();
        }
        text.push_str(&src[last..]);
        unsafe { alloc_str(text) }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn String_split(s: i64, sep: i64) -> i64 {
    if !is_ptr(s) { return new_array(); }
    let src   = unsafe { ptr_to_str(s) };
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep) } } else { " " };
    // Chaîne vide → tableau vide (String.md) ; séparateur vide → un élément
    // par caractère (Rust ajoutait deux éléments vides aux extrémités).
    let parts: Vec<i64> = if src.is_empty() {
        Vec::new()
    } else if sep_s.is_empty() {
        let mut one = [0u8; 4];
        src.chars().map(|c| unsafe { alloc_str(c.encode_utf8(&mut one)) }).collect()
    } else if let [byte] = sep_s.as_bytes() {
        // Séparateur d'un octet : découpe directe sur les octets (un
        // séparateur ASCII ne peut pas couper un caractère UTF-8), en une
        // passe ; place pour 8 morceaux d'emblée (sans réallocation pour une
        // ligne ordinaire).
        let mut parts = Vec::with_capacity(8);
        parts.extend(src.as_bytes().split(|b| b == byte).map(|part| unsafe { alloc_str(std::str::from_utf8_unchecked(part)) }));
        parts
    } else {
        src.split(sep_s).map(|part| unsafe { alloc_str(part) }).collect()
    };
    new_array_with(parts)
}

#[unsafe(no_mangle)]
pub extern "C" fn String_explode(s: i64, sep: i64) -> i64 {
    String_split(s, sep)
}

#[unsafe(no_mangle)]
pub extern "C" fn String_between(s: i64, start: i64, end: i64) -> i64 {
    if !is_ptr(s) { return __rc_passthrough(s); }
    let src     = unsafe { ptr_to_str(s) };
    let start_s = if is_ptr(start) { unsafe { ptr_to_str(start) } } else { "" };
    let end_s   = if is_ptr(end)   { unsafe { ptr_to_str(end) } }   else { "" };
    let r = match src.find(start_s) {
        None => String::new(),
        Some(i) => {
            let after = &src[i + start_s.len()..];
            match after.find(end_s) {
                None    => String::new(),
                Some(j) => after[..j].to_string(),
            }
        }
    };
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_empty(s: i64) -> i64 {
    if !is_ptr(s) { return 1; }
    // Vide ou seulement des blancs (String.md : `String::empty("   ")` → true).
    if unsafe { ptr_to_str(s) }.trim().is_empty() { 1 } else { 0 }
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Math
//
// Codes d'erreur MathException :
//   101 - NEGATIVE_SQRT : Racine carrée d'un nombre négatif
//   102 - NEGATIVE_EXPONENT : Exposant négatif dans pow()
//   103 - INVALID_RANGE : Borne min > max dans random()
// ─────────────────────────────────────────────────────────────────────────────

const ERR_MATH_NEGATIVE_SQRT: i64 = 101;
const ERR_MATH_NEGATIVE_EXPONENT: i64 = 102;
const ERR_MATH_INVALID_RANGE: i64 = 103;
const ERR_MATH_OVERFLOW: i64 = 105;

#[unsafe(no_mangle)]
pub extern "C" fn Math_abs(n: i64) -> i64 {
    n.checked_abs().unwrap_or_else(|| math_overflow(&format!("abs({})", n)))
}

/// Résultat entier hors de `int` : `MathException` 105 (le résultat
/// débordait en silence : `abs(INT_MIN)` = `INT_MIN`, `pow(2, 64)` = 0).
fn math_overflow(op: &str) -> i64 {
    unsafe {
        exception::throw_math_exception(&format!("Integer overflow: {} does not fit in an int", op), ERR_MATH_OVERFLOW, "Math");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Math_min(a: i64, b: i64) -> i64 { a.min(b) }

#[unsafe(no_mangle)]
pub extern "C" fn Math_max(a: i64, b: i64) -> i64 { a.max(b) }

#[unsafe(no_mangle)]
pub extern "C" fn Math_pow(base: i64, exp: i64) -> i64 {
    if exp < 0 {
        unsafe {
            exception::throw_math_exception(
                &format!("Cannot compute power with negative exponent: {}^{}", base, exp),
                ERR_MATH_NEGATIVE_EXPONENT,
                "Math"
            );
        }
    }
    // Exposant au-delà de `u32` : tronqué auparavant (`pow(2, 4294967296)` = 1).
    let checked = u32::try_from(exp).ok().and_then(|e| base.checked_pow(e));
    match (checked, base) {
        (Some(v), _) => v,
        (None, 0 | 1) => base,
        (None, -1) => if exp % 2 == 0 { 1 } else { -1 },
        (None, _) => math_overflow(&format!("pow({}, {})", base, exp)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Math_clamp(n: i64, lo: i64, hi: i64) -> i64 {
    // `i64::clamp` panique si `lo > hi` (abort : panique dans une fonction
    // `extern "C"`) — même exception que `Math::random`.
    if lo > hi {
        unsafe {
            exception::throw_math_exception(
                &format!("Invalid range for clamp: min ({}) > max ({})", lo, hi),
                ERR_MATH_INVALID_RANGE,
                "Math"
            );
        }
    }
    n.clamp(lo, hi)
}

// Symbole attendu par le compilateur : "Math_random" (voir src/codegen/desc.d/
// math.rs) — `Math_` en majuscule comme tous les autres builtins Math_*.
#[unsafe(no_mangle)]
pub extern "C" fn Math_random(min: i64, max: i64) -> i64 {
    if min > max {
        unsafe {
            exception::throw_math_exception(
                &format!("Invalid range for random: min ({}) > max ({})", min, max),
                ERR_MATH_INVALID_RANGE,
                "Math"
            );
        }
    }
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(min..=max)
}

#[unsafe(no_mangle)]
pub extern "C" fn Math_sqrt(n: f64) -> f64 {
    if n < 0.0 {
        unsafe {
            exception::throw_math_exception(
                &format!("Cannot compute square root of negative number: {}", n),
                ERR_MATH_NEGATIVE_SQRT,
                "Math"
            );
        }
    }
    n.sqrt()
}

#[unsafe(no_mangle)]
pub extern "C" fn Math_floor(n: f64) -> i64 { n.floor() as i64 }

#[unsafe(no_mangle)]
pub extern "C" fn Math_ceil(n: f64) -> i64 { n.ceil() as i64 }

#[unsafe(no_mangle)]
pub extern "C" fn Math_round(n: f64) -> i64 {
    // Math.md : 0.5 arrondi vers le haut (`round(-2.5)` → -2), pas loin de zéro.
    (n + 0.5).floor() as i64
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Array
//
// Codes d'erreur ArrayException :
//   101 - EMPTY_ARRAY : Opération sur un tableau vide (pop, first, last)
// ─────────────────────────────────────────────────────────────────────────────

const ERR_ARRAY_EMPTY: i64 = 101;

#[unsafe(no_mangle)]
pub extern "C" fn Array_len(ptr: i64) -> i64 {
    __array_len(ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_push(ptr: i64, val: i64) {
    __array_push(ptr, val)
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_pop(ptr: i64) -> i64 {
    if ptr == 0 {
        unsafe {
            exception::throw_array_exception(
                "Cannot pop from empty array",
                ERR_ARRAY_EMPTY,
                "Array"
            );
        }
    }
    match unsafe { with_array(ptr, |data| data.pop()) } {
        Some(v) => v,
        None => unsafe {
            exception::throw_array_exception("Cannot pop from empty array", ERR_ARRAY_EMPTY, "Array")
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_first(ptr: i64) -> i64 {
    if ptr == 0 {
        unsafe {
            exception::throw_array_exception(
                "Cannot get first element from empty array",
                ERR_ARRAY_EMPTY,
                "Array"
            );
        }
    }
    match unsafe { with_array(ptr, |data| data.first().copied()) } {
        Some(v) => v,
        None => unsafe {
            exception::throw_array_exception("Cannot get first element from empty array", ERR_ARRAY_EMPTY, "Array")
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_last(ptr: i64) -> i64 {
    if ptr == 0 {
        unsafe {
            exception::throw_array_exception(
                "Cannot get last element from empty array",
                ERR_ARRAY_EMPTY,
                "Array"
            );
        }
    }
    match unsafe { with_array(ptr, |data| data.last().copied()) } {
        Some(v) => v,
        None => unsafe {
            exception::throw_array_exception("Cannot get last element from empty array", ERR_ARRAY_EMPTY, "Array")
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_contains(ptr: i64, val: i64) -> i64 {
    if Array_indexOf(ptr, val) >= 0 { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_indexOf(ptr: i64, val: i64) -> i64 {
    if ptr == 0 { return -1; }
    let kind = rc::raw_kind(ptr);
    let found = unsafe {
        with_array(ptr, |data| match kind {
            // Valeur cherchée décodée une fois, puis comparée mot à mot.
            1 => int_needle(val).and_then(|n| data.iter().position(|&x| x == n)),
            _ => data.iter().position(|&x| elem_eq(kind, x, val)),
        })
    };
    found.map_or(-1, |i| i as i64)
}

/// Entier cherché parmi des `int` bruts ; `None` si `needle` n'en est pas un.
fn int_needle(needle: i64) -> Option<i64> {
    (!is_float_box(needle) && !is_bool_box(needle) && get_value_type(needle) == 1).then(|| unbox_numeric_i64(needle))
}

/// Élément `v` d'un conteneur de genre brut `kind` (voir `fmt_elem`) égal à
/// `needle`, valeur dynamique (toujours boxée par l'appelant) : nombres et
/// booléens par valeur, chaînes par contenu. Comparer les mots bruts
/// trouvait une chaîne seulement si c'était le même objet.
fn elem_eq(kind: i64, v: i64, needle: i64) -> bool {
    match kind {
        1 => int_needle(needle) == Some(v),
        2 => is_float_box(needle) && unsafe { unbox_float(needle) } == f64::from_bits(v as u64),
        3 => is_bool_box(needle) && unsafe { unbox_bool(needle) } == (v != 0),
        _ => __cmp_eq_strict(v, needle) == 1,
    }
}

/// Ordre de deux éléments d'un conteneur de genre brut `kind` : nombres par
/// valeur (flottants compris), chaînes par contenu. Trier les mots bruts
/// rangeait les flottants selon leurs bits et les chaînes selon leur adresse.
fn elem_cmp(kind: i64, a: i64, b: i64) -> std::cmp::Ordering {
    match kind {
        2 => f64::from_bits(a as u64).total_cmp(&f64::from_bits(b as u64)),
        1 | 3 => a.cmp(&b),
        _ => dyn_cmp(a, b),
    }
}

/// Ordre de deux valeurs dynamiques : par genre, puis par valeur.
fn dyn_cmp(a: i64, b: i64) -> std::cmp::Ordering {
    let (ta, tb) = (get_value_type(a), get_value_type(b));
    if ta != tb {
        return ta.cmp(&tb);
    }
    match ta {
        1 if is_float_box(a) || is_float_box(b) => unbox_numeric_f64(a).total_cmp(&unbox_numeric_f64(b)),
        1 => unbox_numeric_i64(a).cmp(&unbox_numeric_i64(b)),
        4 => unsafe { ptr_to_str(a).cmp(ptr_to_str(b)) },
        _ => a.cmp(&b),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_reverse(ptr: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let vals = unsafe { with_array(ptr, |data| data.iter().rev().copied().collect()) };
    array_sharing(ptr, vals)
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_slice(ptr: i64, from: i64, to: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let vals = unsafe {
        with_array(ptr, |data| {
            // Bornes ramenées dans [0, len] : un indice négatif valait un
            // `usize` énorme (`slice(a, -2, 5)` vide, `slice(a, 1, -1)` jusqu'à la fin).
            let len = data.len() as i64;
            let lo = from.clamp(0, len) as usize;
            let hi = to.clamp(0, len).max(lo as i64) as usize;
            data[lo..hi].to_vec()
        })
    };
    array_sharing(ptr, vals)
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_join(ptr: i64, sep: i64) -> i64 {
    if ptr == 0 { return unsafe { alloc_str("") }; }
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep) } } else { "" };
    let kind = rc::raw_kind(ptr);
    if let Some(joined) = unsafe { with_array(ptr, |data| join_plain(data, kind, sep_s)) } {
        return joined;
    }
    str_append::with_buffer(|text| {
        for (i, v) in unsafe { array_data(ptr) }.into_iter().enumerate() {
            if i > 0 {
                text.push_str(sep_s);
            }
            match kind {
                0 if typecheck::__is_string(v) != 0 => text.push_str(unsafe { ptr_to_str(v) }),
                _ => text.push_str(&fmt_elem(kind, v)),
            }
        }
        unsafe { alloc_str(text) }
    })
}

/// `Array::join` d'un tableau de chaînes ou d'`int` : écrit directement dans
/// le résultat, de longueur calculée d'avance (ni copie du tableau, ni
/// chaîne intermédiaire) ; `None` pour tout autre élément.
fn join_plain(data: &[i64], kind: i64, sep: &str) -> Option<i64> {
    let text_len = |v: i64| match kind {
        1 => Some(str_append::int_len(v)),
        0 if typecheck::__is_string(v) != 0 => Some(unsafe { str_append::str_len(v) }),
        _ => None,
    };
    let mut len = sep.len() * data.len().saturating_sub(1);
    for &v in data {
        len += text_len(v)?;
    }
    Some(unsafe {
        alloc_str_with(len, |out| {
            let mut at = 0;
            for (i, &v) in data.iter().enumerate() {
                if i > 0 {
                    out[at..at + sep.len()].copy_from_slice(sep.as_bytes());
                    at += sep.len();
                }
                let digits = &mut [0; 20];
                let text = if kind == 1 { int_text(v, digits) } else { ptr_to_str(v) };
                out[at..at + text.len()].copy_from_slice(text.as_bytes());
                at += text.len();
            }
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_sort(ptr: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let kind = rc::raw_kind(ptr);
    let mut vals = unsafe { array_data(ptr) };
    vals.sort_by(|&a, &b| elem_cmp(kind, a, b));
    array_sharing(ptr, vals)
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_get(ptr: i64, idx: i64) -> i64 {
    __array_get(ptr, idx)
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_set(ptr: i64, idx: i64, val: i64) {
    __array_set(ptr, idx, val)
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Map
//
// Codes d'erreur MapException :
//   101 - KEY_NOT_FOUND : Clé inexistante (get)
// ─────────────────────────────────────────────────────────────────────────────

const ERR_MAP_KEY_NOT_FOUND: i64 = 101;

#[unsafe(no_mangle)]
pub extern "C" fn Map_size(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { with_map(ptr, |data| data.len() as i64) }
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_has(ptr: i64, key: i64) -> i64 {
    if ptr == 0 { return 0; }
    if unsafe { with_key_text(key, |k| with_map(ptr, |data| data.contains_key(Key::Text(k)))) } { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_get(ptr: i64, key: i64) -> i64 {
    if ptr == 0 {
        unsafe {
            let key_str = if is_ptr(key) { ptr_to_str(key).to_string() } else { key.to_string() };
            exception::throw_map_exception(
                &format!("Key not found: {}", key_str),
                ERR_MAP_KEY_NOT_FOUND,
                "Map"
            );
        }
    }
    let k = unsafe { key_to_string(key) };
    match unsafe { with_map(ptr, |data| data.get(Key::Text(&k))) } {
        Some(v) => v,
        None => unsafe {
            exception::throw_map_exception(&format!("Key not found: {}", k), ERR_MAP_KEY_NOT_FOUND, "Map")
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_set(ptr: i64, key: i64, val: i64) {
    __map_set(ptr, key, val);
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_remove(ptr: i64, key: i64) {
    if ptr == 0 { return; }
    let removed = unsafe { with_key_text(key, |k| with_map(ptr, |data| data.remove(Key::Text(k)))) };
    if let Some(v) = removed {
        release_elem(ptr, v);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_keys(ptr: i64, kind: i64) -> i64 {
    let arr_ptr = new_array();
    // `kind` (argument caché, type de clé `K` connu du compilateur) : clés
    // `int`/`float`/`bool` rendues brutes, tableau marqué en conséquence.
    if (1..=3).contains(&kind) {
        crate::rc::__rc_mark_raw_kind(arr_ptr, kind);
    }
    if ptr == 0 { return arr_ptr; }
    // Les clés sont stockées en texte — converties au type `K`.
    let keys: Vec<i64> = unsafe {
        with_map(ptr, |data| data.keys().map(|k| match (k, kind) {
            (KeyRef::Int(value), 1) => value,
            (k, 1..=3) => k.with_text(|text| key_from_text(text, kind)),
            (k, _) => k.share(),
        }).collect())
    };
    unsafe { with_array(arr_ptr, |data| *data = keys) }
    arr_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_values(ptr: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let vals = unsafe { map_values(ptr) };
    array_sharing(ptr, vals)
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_merge(a: i64, b: i64) -> i64 {
    let new_ptr = new_map();
    for src in [a, b] {
        for (k, v) in unsafe { map_entries(src) } {
            retain_elem(src, v);
            map_set_owned_key(new_ptr, unsafe { alloc_str(&k) }, v);
        }
    }
    new_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_isEmpty(ptr: i64) -> i64 {
    if ptr == 0 { return 1; }
    if unsafe { with_map(ptr, |data| data.is_empty()) } { 1 } else { 0 }
}

/// Map::forEach(m, callback) → void
/// `callback` : nameless(key:mixed, value:mixed): void, appelé pour chaque
/// entrée. `callback` est un fat pointer Ocara {func_ptr, env_ptr} (même
/// convention que les handlers HTTPServer::route, voir runtime/src/httpserver.rs).
#[unsafe(no_mangle)]
///
/// `shape` : forme des valeurs connue du compilateur (`static_leaf_shape`) —
/// une valeur brute `int`/`float`/`bool` est boxée avant d'être passée au
/// paramètre `mixed` de la closure (sinon lue comme un pointeur : SIGSEGV).
pub extern "C" fn Map_forEach(ptr: i64, callback: i64, shape: i64) {
    if ptr == 0 || callback == 0 { return; }
    type ForEachFn = extern "C" fn(i64, i64, i64) -> i64;
    let func_ptr = unsafe { *(callback as *const i64) };
    let env_ptr  = unsafe { *(callback as *const i64).add(1) };
    let f: ForEachFn = unsafe { std::mem::transmute(func_ptr as usize) };
    // Copie des entrées avant d'itérer : le callback pourrait modifier la map
    // (Map::set/remove) pendant l'itération, ce qui invaliderait une référence
    // directe vers `data`.
    let entries: Vec<(String, i64)> = unsafe { map_entries(ptr) };
    for (_, v) in &entries { retain_elem(ptr, *v) }
    let leaf_kind = if shape >> 8 == 1 { shape & 0xff } else { rc::raw_kind(ptr) };
    for (k, v) in &entries {
        let key_ptr = unsafe { alloc_str(k) };
        let value = match leaf_kind {
            1 => __box_int_for_mixed(*v),
            2 => __box_float(*v),
            3 => __box_bool(*v),
            _ => *v,
        };
        f(env_ptr, key_ptr, value);
        if value != *v { unsafe { rc::release(value) } }
        unsafe { rc::release(key_ptr) }
    }
    for (_, v) in &entries { release_elem(ptr, *v) }
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Convert
//
// Codes d'erreur ConvertException :
//   101 - INVALID_INT   : Conversion string vers int invalide
//   102 - INVALID_FLOAT : Conversion string vers float invalide
//   103 - TYPE          : Convert::mixedToStr sur un tableau, une map ou un objet
// ─────────────────────────────────────────────────────────────────────────────

const ERR_CONVERT_INVALID_INT: i64 = 101;
const ERR_CONVERT_INVALID_FLOAT: i64 = 102;
/// Valeur `mixed` qui n'est ni une chaîne ni un scalaire (Convert::mixedToStr).
pub(crate) const ERR_CONVERT_TYPE: i64 = 103;

#[unsafe(no_mangle)]
pub extern "C" fn Convert_strToInt(s: i64) -> i64 {
    if !is_ptr(s) { return 0; }
    let src = unsafe { ptr_to_str(s).trim() };
    match src.parse::<i64>() {
        Ok(n) => n,
        Err(_) => unsafe {
            exception::throw_convert_exception(
                &format!("Cannot convert string to int: '{}'", src),
                ERR_CONVERT_INVALID_INT,
                "Convert"
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_strToFloat(s: i64) -> f64 {
    if !is_ptr(s) { return 0.0; }
    let src = unsafe { ptr_to_str(s).trim() };
    match src.parse::<f64>() {
        Ok(f) => f,
        Err(_) => unsafe {
            exception::throw_convert_exception(
                &format!("Cannot convert string to float: '{}'", src),
                ERR_CONVERT_INVALID_FLOAT,
                "Convert"
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_strToBool(s: i64) -> i64 {
    if !is_ptr(s) { return 0; }
    let t = unsafe { ptr_to_str(s).trim().to_lowercase() };
    if t == "true" || t == "1" { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_strToArray(s: i64, sep: i64) -> i64 {
    String_split(s, sep)
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_strToMap(s: i64, sep: i64, kv: i64) -> i64 {
    if !is_ptr(s) { return new_map(); }
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep).to_string() } } else { ",".to_string() };
    let kv_s  = if is_ptr(kv)  { unsafe { ptr_to_str(kv).to_string() }  } else { "=".to_string() };
    let src = unsafe { ptr_to_str(s).to_string() };
    let ptr = new_map();
    for part in src.split(sep_s.as_str()) {
        if let Some(pos) = part.find(kv_s.as_str()) {
            let v_str = unsafe { alloc_str(&part[pos + kv_s.len()..]) };
            unsafe { map_set_owned_key(ptr, alloc_str(&part[..pos]), v_str); }
        }
    }
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_mixedToStr(val: i64) -> i64 {
    if val == 0 {
        return 0;
    }
    if is_float_box(val) || is_bool_box(val) || is_int_box(val) || !is_ptr(val) {
        return unsafe { alloc_str(&val_to_string(val)) };
    }
    if crate::typecheck::__is_string(val) != 0 {
        return __rc_passthrough(val);
    }
    unsafe {
        crate::exception::throw_convert_exception(
            "Convert::mixedToStr: an array, map or object cannot be used as a string",
            ERR_CONVERT_TYPE,
            "Convert",
        )
    }
}

/// mixed → string — voir `Convert_mixedToStr` plus haut.
#[unsafe(no_mangle)]
pub extern "C" fn Convert_intToStr(n: i64) -> i64 {
    unsafe { alloc_str(int_text(n, &mut [0; 20])) }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_intToFloat(n: i64) -> f64 {
    n as f64
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_intToBool(n: i64) -> i64 {
    if n != 0 { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_floatToStr(f: f64) -> i64 {
    unsafe { alloc_str(&float_text(f)) }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_floatToInt(f: f64) -> i64 {
    f as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_floatToBool(f: f64) -> i64 {
    if f != 0.0 { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_boolToStr(b: i64) -> i64 {
    unsafe { alloc_str(if b != 0 { "true" } else { "false" }) }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_boolToInt(b: i64) -> i64 {
    if b != 0 { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_boolToFloat(b: i64) -> f64 {
    if b != 0 { 1.0 } else { 0.0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_arrayToStr(ptr: i64, sep: i64) -> i64 {
    Array_join(ptr, sep)
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_arrayToMap(ptr: i64, kv: i64) -> i64 {
    if ptr == 0 { return new_map(); }
    let kv_s = if is_ptr(kv) { unsafe { ptr_to_str(kv).to_string() } } else { "=".to_string() };
    let map_ptr = new_map();
    unsafe {
        for elem in array_data(ptr) {
            if is_ptr(elem) {
                let s = ptr_to_str(elem).to_string();
                if let Some(pos) = s.find(kv_s.as_str()) {
                    let v = alloc_str(&s[pos + kv_s.len()..]);
                    map_set_owned_key(map_ptr, alloc_str(&s[..pos]), v);
                }
            }
        }
    }
    map_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_mapToStr(ptr: i64, sep: i64, kv: i64) -> i64 {
    if ptr == 0 { return unsafe { alloc_str("") }; }
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep).to_string() } } else { ",".to_string() };
    let kv_s  = if is_ptr(kv)  { unsafe { ptr_to_str(kv).to_string() }  } else { "=".to_string() };
    let parts: Vec<String> = unsafe { map_entries(ptr) }.iter()
        .map(|(k, v)| format!("{}{}{}", k, kv_s, fmt_val(*v)))
        .collect();
    let r = parts.join(&sep_s);
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_mapKeysToArray(ptr: i64) -> i64 {
    Map_keys(ptr, 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_mapValuesToArray(ptr: i64) -> i64 {
    Map_values(ptr)
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.System
//
// Codes d'erreur SystemException :
//   101 - EXEC    : Erreur d'exécution de commande
//   102 - CWD     : Erreur de lecture du répertoire courant
//   103 - SET_ENV : Erreur de définition de variable d'environnement
// ─────────────────────────────────────────────────────────────────────────────

const ERR_SYSTEM_EXEC: i64 = 101;
const ERR_SYSTEM_CWD: i64 = 102;
const ERR_SYSTEM_SET_ENV: i64 = 103;

#[unsafe(no_mangle)]
pub extern "C" fn System_exec(cmd: i64) -> i64 {
    if !is_ptr(cmd) { return unsafe { alloc_str("") }; }
    let cmd_s = unsafe { ptr_to_str(cmd) };
    match Command::new("sh")
        .arg("-c")
        .arg(cmd_s)
        .output()
    {
        Ok(output) => {
            let mut out = String::from_utf8_lossy(&output.stdout).to_string();
            if out.ends_with('\n') { out.pop(); }
            unsafe { alloc_str(&out) }
        }
        Err(e) => unsafe {
            exception::throw_system_exception(
                &format!("Failed to execute command '{}': {}", cmd_s, e),
                ERR_SYSTEM_EXEC,
                "System"
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn System_passthrough(cmd: i64) -> i64 {
    if !is_ptr(cmd) { return 0; }
    let cmd_s = unsafe { ptr_to_str(cmd) };
    match Command::new("sh")
        .arg("-c")
        .arg(cmd_s)
        .status()
    {
        Ok(status) => status.code().unwrap_or(1) as i64,
        Err(e) => unsafe {
            exception::throw_system_exception(
                &format!("Failed to execute command '{}': {}", cmd_s, e),
                ERR_SYSTEM_EXEC,
                "System"
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn System_execCode(cmd: i64) -> i64 {
    System_passthrough(cmd)
}

#[unsafe(no_mangle)]
pub extern "C" fn System_exit(code: i64) {
    std::process::exit(code as i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn System_env(name: i64) -> i64 {
    if !is_ptr(name) { return unsafe { alloc_str("") }; }
    let key = unsafe { ptr_to_str(name) };
    let val = std::env::var(key).unwrap_or_default();
    unsafe { alloc_str(&val) }
}

#[unsafe(no_mangle)]
pub extern "C" fn System_setEnv(name: i64, val: i64) {
    if !is_ptr(name) { return; }
    let key = unsafe { ptr_to_str(name) };
    let v   = if is_ptr(val) { unsafe { ptr_to_str(val).to_string() } } else { val.to_string() };
    
    // `set_var` panique sur un nom vide ou contenant '=' / NUL, ou une valeur
    // contenant NUL : vérifié avant, sans trace de panique sur stderr.
    if !valid_env_pair(key, &v) {
        unsafe {
            exception::throw_system_exception(
                &format!("Failed to set environment variable '{}': invalid name or value", key),
                ERR_SYSTEM_SET_ENV,
                "System"
            );
        }
    }
    unsafe { std::env::set_var(key, &v); }
}

/// Nom et valeur acceptés par `std::env::set_var` sans panique.
pub(crate) fn valid_env_pair(key: &str, value: &str) -> bool {
    !key.is_empty() && !key.contains('=') && !key.contains('\0') && !value.contains('\0')
}

#[unsafe(no_mangle)]
pub extern "C" fn System_cwd() -> i64 {
    match std::env::current_dir() {
        Ok(path) => {
            let path_str = path.to_string_lossy().to_string();
            unsafe { alloc_str(&path_str) }
        }
        Err(e) => unsafe {
            exception::throw_system_exception(
                &format!("Failed to get current working directory: {}", e),
                ERR_SYSTEM_CWD,
                "System"
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn System_sleep(ms: i64) {
    let _parked = rc::park();
    // Durée négative : aucune attente (convertie en `u64`, elle durait des
    // milliards d'années).
    std::thread::sleep(Duration::from_millis(ms.max(0) as u64));
}

#[unsafe(no_mangle)]
pub extern "C" fn System_pid() -> i64 {
    std::process::id() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn System_args() -> i64 {
    let ptr = new_array();
    let args: Vec<i64> = std::env::args().map(|arg| unsafe { alloc_str(&arg) }).collect();
    unsafe { with_array(ptr, |data| *data = args) }
    ptr
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Regex — implémentation via la crate `regex`
//
// Codes d'erreur RegexException :
//   101 - INVALID_PATTERN : Pattern regex invalide (erreur de syntaxe)
// ─────────────────────────────────────────────────────────────────────────────

use regex::Regex as Re;

const ERR_REGEX_INVALID_PATTERN: i64 = 101;

/// Motifs déjà compilés (par thread) : un motif écrit dans une boucle était
/// recompilé à chaque appel (30 µs). Partagés par `Rc` : un clone de `Regex`
/// recrée son cache de recherche. Vidé quand il atteint la limite.
const REGEX_CACHE_LIMIT: usize = 256;

thread_local! {
    static REGEX_CACHE: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<Re>>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Compile le pattern (i64 ptr → &str) ; lève RegexException si invalide.
unsafe fn compile_regex(pattern: i64) -> std::rc::Rc<Re> {
    unsafe {
        let pat = ptr_to_str(pattern);
        if let Some(re) = REGEX_CACHE.with(|c| c.borrow().get(pat).cloned()) {
            return re;
        }
        match Re::new(pat) {
            Ok(re) => {
                let re = std::rc::Rc::new(re);
                REGEX_CACHE.with(|c| {
                    let mut cache = c.borrow_mut();
                    if cache.len() >= REGEX_CACHE_LIMIT {
                        cache.clear();
                    }
                    cache.insert(pat.to_string(), re.clone());
                });
                re
            }
            Err(e) => {
                exception::throw_regex_exception(
                    &format!("Invalid regex pattern: '{}' ({})", pat, e),
                    ERR_REGEX_INVALID_PATTERN,
                    "Regex"
                );
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Regex_test(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        if re.is_match(s) { 1 } else { 0 }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Regex_find(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        match re.find(s) {
            Some(m) => alloc_str(m.as_str()),
            None    => alloc_str(""),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Regex_findAll(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        let arr = new_array();
        for m in re.find_iter(s) {
            let ms = alloc_str(m.as_str());
            array_push_owned(arr, ms);
        }
        arr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Regex_replace(pattern: i64, text: i64, repl: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        let r  = ptr_to_str(repl);
        let result = re.replacen(s, 1, r).into_owned();
        alloc_str(&result)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Regex_replaceAll(pattern: i64, text: i64, repl: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        let r  = ptr_to_str(repl);
        let result = re.replace_all(s, r).into_owned();
        alloc_str(&result)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Regex_split(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        let arr = new_array();
        for part in re.split(s) {
            let ps = alloc_str(part);
            array_push_owned(arr, ps);
        }
        arr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Regex_count(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        re.find_iter(s).count() as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Regex_extract(pattern: i64, text: i64, n: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        match re.captures(s) {
            Some(caps) => match caps.get(n as usize) {
                Some(m) => alloc_str(m.as_str()),
                None    => alloc_str(""),
            },
            None => alloc_str(""),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPRequest — implémenté dans httprequest.rs
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// ocara.UnitTest — assertions pour les tests unitaires
// ─────────────────────────────────────────────────────────────────────────────

/// Affiche PASS ou FAIL sur stderr, avec un message optionnel.
/// Appelé par ocaraunit — chaque assertion notifie son résultat via stdout.
///
/// Format stdout (pour ocaraunit) :
///   PASS <message>
///   FAIL <message>
pub(crate) fn ut_pass(msg: &str) {
    unittest_runtime::report(&format!("PASS {}", msg));
}

unsafe fn ut_val_to_display(val: i64) -> String {
    // Ancienne version : réimplémentation ad-hoc du déballage float/bool
    // boxé, ET FAUSSE (traitait `val` comme encodant directement les bits du
    // float décalés de 2, alors que le boxing alloue une VRAIE cellule tas
    // et retourne `ptr | tag` — voir `__box_float`/`unbox_float`). Corrigé en
    // délégant à `val_to_string`, déjà correcte pour float/bool/int boxés
    // (voir `is_float_box`/`is_bool_box`/`is_int_box`) — seul l'habillage
    // entre guillemets d'une vraie string est spécifique à cette fonction.
    if val == 0 {
        return "null".to_string();
    }
    if __is_string(val) != 0 {
        return format!("\"{}\"", unsafe { ptr_to_str(val) });
    }
    val_to_string(val)
}

/// Vide au sens de `assertEmpty` : `null`, chaîne, tableau ou map sans
/// élément. Un tableau était lu comme une chaîne (`assertEmpty([])` échouait).
fn is_empty_value(value: i64) -> bool {
    if value == 0 {
        return true;
    }
    match get_value_type(value) {
        4 => unsafe { ptr_to_str(value).is_empty() },
        5 => unsafe { array_data(value).is_empty() },
        6 => unsafe { map_entries(value).is_empty() },
        _ => false,
    }
}

/// Égalité de `assertEquals` : contenu des tableaux et des maps (éléments
/// bruts selon leur genre), sinon `__cmp_eq_strict`. Deux tableaux de même
/// contenu étaient comparés par adresse.
fn values_deep_eq(a: i64, b: i64, depth: usize) -> bool {
    let (ta, tb) = (get_value_type(a), get_value_type(b));
    if depth > 16 || ta != tb || !matches!(ta, 5 | 6) {
        return __cmp_eq_strict(a, b) != 0;
    }
    let (ka, kb) = (rc::raw_kind(a), rc::raw_kind(b));
    if ta == 5 {
        let (xs, ys) = unsafe { (array_data(a), array_data(b)) };
        return xs.len() == ys.len() && xs.iter().zip(&ys).all(|(&x, &y)| elems_deep_eq(ka, x, kb, y, depth));
    }
    let (xs, ys) = unsafe { (map_entries(a), map_entries(b)) };
    xs.len() == ys.len() && xs.iter().all(|(k, x)| ys.iter().any(|(k2, y)| k == k2 && elems_deep_eq(ka, *x, kb, *y, depth)))
}

/// Deux éléments de conteneurs de genres bruts `ka` et `kb`.
fn elems_deep_eq(ka: i64, x: i64, kb: i64, y: i64, depth: usize) -> bool {
    match (ka, kb) {
        (0, 0) => values_deep_eq(x, y, depth + 1),
        (0, k) => elem_eq(k, y, x),
        (k, 0) => elem_eq(k, x, y),
        (2, 2) => f64::from_bits(x as u64) == f64::from_bits(y as u64),
        (k1, k2) => k1 == k2 && x == y,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertEquals(expected: i64, actual: i64) {
    // Comparaison RÉELLE (type + valeur, contenu pour une string), pas une
    // égalité brute de i64 — celle-ci ne "marchait" pour deux strings que
    // par coïncidence (mêmes pointeurs, ex. deux littéraux internés
    // identiques), et confondait deux valeurs `mixed` boxées distinctes
    // représentant le même nombre (deux adresses de cellule différentes) —
    // découvert et documenté plus tôt dans ce chantier, corrigé ici en
    // réutilisant `__cmp_eq_strict` (déjà correcte pour tous les cas, voir
    // sa doc plus bas dans ce fichier).
    if values_deep_eq(expected, actual, 0) {
        ut_pass(&format!("assertEquals: {} == {}",
            unsafe { ut_val_to_display(expected) }, unsafe { ut_val_to_display(actual) }));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertEquals: expected {} but got {}",
                    ut_val_to_display(expected), ut_val_to_display(actual)),
                101
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertNotEquals(expected: i64, actual: i64) {
    // Voir le commentaire de `UnitTest_assertEquals` — même correction.
    if !values_deep_eq(expected, actual, 0) {
        ut_pass(&format!("assertNotEquals: {} != {}",
            unsafe { ut_val_to_display(expected) }, unsafe { ut_val_to_display(actual) }));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertNotEquals: values are equal ({})", ut_val_to_display(actual)),
                102
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertTrue(value: i64) {
    // Déballer un bool/int/float boxé (voir `unbox_numeric_i64`) avant le
    // test de vérité : un bool boxé est un pointeur heap, donc TOUJOURS non
    // nul, qu'il représente `true` OU `false` — sans déballage,
    // `assertTrue(false)` passerait à tort dès que son argument est un
    // `mixed` boxé (paramètre déclaré `Type::Mixed`, voir
    // src/builtins/unittest.rs, et le boxing d'argument d'appel introduit
    // dans ce chantier — voir docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md).
    if unbox_numeric_i64(value) != 0 {
        ut_pass("assertTrue");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertTrue: value is false", 103);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertFalse(value: i64) {
    // Voir le commentaire de `UnitTest_assertTrue` — même correction.
    if unbox_numeric_i64(value) == 0 {
        ut_pass("assertFalse");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertFalse: value is true", 104);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertNull(value: i64) {
    if value == 0 {
        ut_pass("assertNull");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertNull: value is not null", 105);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertNotNull(value: i64) {
    if value != 0 {
        ut_pass("assertNotNull");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertNotNull: value is null", 106);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertGreater(a: i64, b: i64) {
    // `cmp_primitive` (voir plus bas) déballe un opérande float/bool/int
    // boxé avant de comparer — une comparaison brute de bits (l'ancien
    // comportement) donnait un résultat sans rapport avec la valeur réelle
    // pour un `mixed` boxé (params déclarés `Type::Mixed`, voir
    // src/builtins/unittest.rs).
    if cmp_primitive(a, b, |x, y| x > y, |x, y| x > y) {
        ut_pass(&format!("assertGreater: {} > {}", unsafe { ut_val_to_display(a) }, unsafe { ut_val_to_display(b) }));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertGreater: {} is not > {}", ut_val_to_display(a), ut_val_to_display(b)),
                107
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertLess(a: i64, b: i64) {
    if cmp_primitive(a, b, |x, y| x < y, |x, y| x < y) {
        ut_pass(&format!("assertLess: {} < {}", unsafe { ut_val_to_display(a) }, unsafe { ut_val_to_display(b) }));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertLess: {} is not < {}", ut_val_to_display(a), ut_val_to_display(b)),
                108
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertGreaterOrEquals(a: i64, b: i64) {
    if cmp_primitive(a, b, |x, y| x >= y, |x, y| x >= y) {
        ut_pass(&format!("assertGreaterOrEquals: {} >= {}", unsafe { ut_val_to_display(a) }, unsafe { ut_val_to_display(b) }));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertGreaterOrEquals: {} is not >= {}", ut_val_to_display(a), ut_val_to_display(b)),
                109
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertLessOrEquals(a: i64, b: i64) {
    if cmp_primitive(a, b, |x, y| x <= y, |x, y| x <= y) {
        ut_pass(&format!("assertLessOrEquals: {} <= {}", unsafe { ut_val_to_display(a) }, unsafe { ut_val_to_display(b) }));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertLessOrEquals: {} is not <= {}", ut_val_to_display(a), ut_val_to_display(b)),
                110
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertContains(haystack: i64, needle: i64) {
    // Même risque que `UnitTest_assertEmpty`/`assertNotEmpty` (déjà corrigé) :
    // une valeur boxée (float/bool/int — voir `is_float_box`/`is_bool_box`/
    // `is_int_box`) est un pointeur heap dont les bits bas ne sont pas ceux
    // d'une vraie string — `ptr_to_str` dessus lirait une zone mémoire
    // arbitraire (même famille de SEGFAULT que documentée dans
    // docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md).
    if is_float_box(haystack) || is_bool_box(haystack) || is_int_box(haystack)
        || is_float_box(needle) || is_bool_box(needle) || is_int_box(needle) {
        unsafe {
            crate::exception::throw_unittest_exception(
                "assertContains: haystack/needle is not a string (boxed mixed value)",
                111
            );
        }
    }
    unsafe {
        let h = ptr_to_str(haystack);
        let n = ptr_to_str(needle);
        if h.contains(n) {
            ut_pass(&format!("assertContains: \"{}\" contains \"{}\"", h, n));
        } else {
            crate::exception::throw_unittest_exception(
                &format!("assertContains: \"{}\" does not contain \"{}\"", h, n),
                111
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertEmpty(value: i64) {
    // Un bool/int/float boxé (voir `is_float_box`/`is_bool_box`/`is_int_box`)
    // est un pointeur heap dont les bits bas NE SONT PAS ceux d'une vraie
    // string (voir `is_ptr`) — jamais "vide" au sens de cette assertion,
    // et `ptr_to_str` sur son adresse (décalée du tag) lirait une zone
    // mémoire arbitraire (SEGFAULT potentiel, même famille de bug que
    // docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md).
    let empty = is_empty_value(value);
    if empty {
        ut_pass("assertEmpty");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertEmpty: value is not empty", 112);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertNotEmpty(value: i64) {
    // Voir le commentaire de `UnitTest_assertEmpty` — même correction.
    let empty = is_empty_value(value);
    if !empty {
        ut_pass("assertNotEmpty");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertNotEmpty: value is empty", 113);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_fail(message: i64) {
    unsafe {
        let msg = ptr_to_str(message);
        crate::exception::throw_unittest_exception(msg, 114);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_pass(message: i64) {
    unsafe {
        let msg = ptr_to_str(message);
        ut_pass(msg);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertFunction(value: i64) {
    let is_func = __is_function(value) != 0;
    if is_func {
        ut_pass("assertFunction");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertFunction: value is not a function", 115);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertClass(value: i64) {
    let is_obj = __is_object(value) != 0;
    if is_obj {
        ut_pass("assertClass");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertClass: value is not a class instance", 116);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertEnum(_value: i64) {
    // Une variante d'enum est un entier à l'exécution : le type est vérifié
    // à la compilation (la sema refuse tout argument qui n'est pas une
    // valeur d'enum), l'assertion réussit donc ici.
    ut_pass("assertEnum");
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertMap(value: i64) {
    let is_map = __is_map(value) != 0;
    if is_map {
        ut_pass("assertMap");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertMap: value is not a map", 118);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertArray(value: i64) {
    let is_arr = __is_array(value) != 0;
    if is_arr {
        ut_pass("assertArray");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertArray: value is not an array", 119);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// assertRaises : mécanisme de capture d'exception pour les tests
// ─────────────────────────────────────────────────────────────────────────────

/// Signature d'une closure Ocara : fn(env_ptr: i64) -> i64
type OcaraClosureFn = unsafe extern "C" fn(i64) -> i64;

thread_local! {
    static ASSERT_RAISES_FUNC_PTR: Cell<i64> = Cell::new(0);
    static ASSERT_RAISES_ENV_PTR: Cell<i64> = Cell::new(0);
    static ASSERT_RAISES_EXCEPTION: Cell<i64> = Cell::new(0);
    static ASSERT_RAISES_EXCEPTION_TYPE: Cell<i64> = Cell::new(0);
}

#[unsafe(no_mangle)]
extern "C" fn __assert_raises_body() -> i64 {
    let func_ptr = ASSERT_RAISES_FUNC_PTR.with(|c| c.get());
    let env_ptr = ASSERT_RAISES_ENV_PTR.with(|c| c.get());
    
    if func_ptr != 0 {
        unsafe {
            let f: OcaraClosureFn = std::mem::transmute(func_ptr as usize);
            f(env_ptr);
        }
    }
    TRY_NORMAL
}

#[unsafe(no_mangle)]
extern "C" fn __assert_raises_handler(error_val: i64, error_type: i64) -> i64 {
    unsafe { rc::retain(error_val) }
    ASSERT_RAISES_EXCEPTION.with(|e| e.set(error_val));
    ASSERT_RAISES_EXCEPTION_TYPE.with(|t| t.set(error_type));
    TRY_NORMAL
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertRaises(fat_ptr: i64) -> i64 {
    // Extraire func_ptr et env_ptr du fat pointer
    let func_ptr = unsafe { *(fat_ptr as *const i64) };
    let env_ptr  = unsafe { *((fat_ptr as *const i64).add(1)) };
    
    // Stocker dans les thread_locals
    ASSERT_RAISES_FUNC_PTR.with(|c| c.set(func_ptr));
    ASSERT_RAISES_ENV_PTR.with(|c| c.set(env_ptr));
    ASSERT_RAISES_EXCEPTION.with(|e| e.set(0));
    ASSERT_RAISES_EXCEPTION_TYPE.with(|t| t.set(0));
    
    // Exécuter le callable dans un contexte try
    let body_addr = __assert_raises_body as *const () as usize as i64;
    let handler_addr = __assert_raises_handler as *const () as usize as i64;
    __ocara_try_exec(body_addr, handler_addr);
    
    // Récupérer l'exception capturée
    let exception = ASSERT_RAISES_EXCEPTION.with(|e| e.get());
    
    if exception == 0 {
        // Aucune exception levée
        unsafe {
            crate::exception::throw_unittest_exception(
                "assertRaises: callable did not raise an exception",
                120
            );
        }
    }
    
    ut_pass("assertRaises");
    exception
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertExceptionMessageEquals(message: i64, expected: i64) {
    unsafe {
        let msg_str = if is_ptr(message) { ptr_to_str(message) } else { "" };
        let exp_str = if is_ptr(expected) { ptr_to_str(expected) } else { "" };
        if msg_str == exp_str {
            ut_pass(&format!("assertExceptionMessageEquals: \"{}\" == \"{}\"", msg_str, exp_str));
        } else {
            crate::exception::throw_unittest_exception(
                &format!("assertExceptionMessageEquals: attendu \"{}\" mais obtenu \"{}\"", exp_str, msg_str),
                121
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertExceptionMessageNotEquals(message: i64, expected: i64) {
    unsafe {
        let msg_str = if is_ptr(message) { ptr_to_str(message) } else { "" };
        let exp_str = if is_ptr(expected) { ptr_to_str(expected) } else { "" };
        if msg_str != exp_str {
            ut_pass(&format!("assertExceptionMessageNotEquals: \"{}\" != \"{}\"", msg_str, exp_str));
        } else {
            crate::exception::throw_unittest_exception(
                &format!("assertExceptionMessageNotEquals: ne devait pas être égal à \"{}\"", exp_str),
                122
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertExceptionCodeEquals(code: i64, expected: i64) {
    if code == expected {
        ut_pass(&format!("assertExceptionCodeEquals: {} == {}", code, expected));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertExceptionCodeEquals: attendu {} mais obtenu {}", expected, code),
                123
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertExceptionCodeNotEquals(code: i64, expected: i64) {
    if code != expected {
        ut_pass(&format!("assertExceptionCodeNotEquals: {} != {}", code, expected));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertExceptionCodeNotEquals: ne devait pas être égal à {}", expected),
                124
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertExceptionSourceEquals(source: i64, expected: i64) {
    unsafe {
        let src_str = if is_ptr(source) { ptr_to_str(source) } else { "" };
        let exp_str = if is_ptr(expected) { ptr_to_str(expected) } else { "" };
        if src_str == exp_str {
            ut_pass(&format!("assertExceptionSourceEquals: \"{}\" == \"{}\"", src_str, exp_str));
        } else {
            crate::exception::throw_unittest_exception(
                &format!("assertExceptionSourceEquals: attendu \"{}\" mais obtenu \"{}\"", exp_str, src_str),
                125
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertExceptionSourceNotEquals(source: i64, expected: i64) {
    unsafe {
        let src_str = if is_ptr(source) { ptr_to_str(source) } else { "" };
        let exp_str = if is_ptr(expected) { ptr_to_str(expected) } else { "" };
        if src_str != exp_str {
            ut_pass(&format!("assertExceptionSourceNotEquals: \"{}\" != \"{}\"", src_str, exp_str));
        } else {
            crate::exception::throw_unittest_exception(
                &format!("assertExceptionSourceNotEquals: ne devait pas être égal à \"{}\"", exp_str),
                126
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Error handling — Rust-side runtime wrappers
// (the real setjmp/longjmp mechanism is in try_impl.c)
// ─────────────────────────────────────────────────────────────────────────────

/// Unhandled exception: called if `raise` is thrown outside any `try`.
/// Printed in red on stderr, then exit(1).
/// In practice, __ocara_fail in try_impl.c calls this logic in C.
/// This Rust symbol serves as fallback / documentation.
#[unsafe(no_mangle)]
pub extern "C" fn __ocara_unhandled_fail(_val: i64, type_name: i64) {
    let type_str = unsafe { ptr_to_str(type_name) }.to_string();
    let max_width = 46;
    let char_count = type_str.chars().count();
    
    // Tronquer et padder manuellement pour gérer correctement les caractères Unicode
    let display_val = if char_count > max_width {
        let truncated: String = type_str.chars().take(max_width - 3).collect();
        let padding = " ".repeat(max_width - (max_width - 3) - 3);  // padding pour "..."
        format!("{}...{}", truncated, padding)
    } else {
        let padding = " ".repeat(max_width - char_count);
        format!("{}{}", type_str, padding)
    };
    
    let msg = format!(
        "\x1b[31m╔═════════════════════════════════════════════════════════════════╗\n\
         ║ UNHANDLED EXCEPTION                                             ║\n\
         ╠═════════════════════════════════════════════════════════════════╣\n\
         ║ Exception raised: {}║\n\
         ║                                                                 ║\n\
         ║ No try/on block found to catch this exception.                  ║\n\
         ║                                                                 ║\n\
         ║ Solutions:                                                      ║\n\
         ║  • Wrap the code in a try/on block to catch exceptions          ║\n\
         ║  • Run unit tests with 'ocaraunit' instead of direct execution  ║\n\
         ╚═════════════════════════════════════════════════════════════════╝\x1b[0m\n",
        display_val
    );
    write_stderr_raw(msg.as_bytes());
    std::process::exit(1);
}

/// Alloue `size` octets sans tag — pour les closures env et allocations internes.
#[unsafe(no_mangle)]
pub extern "C" fn __alloc_obj(size: i64) -> i64 {
    if size <= 0 { return 0; }
    unsafe {
        let layout = Layout::from_size_align(size as usize, 8).unwrap();
        let ptr = alloc_zeroed(layout);
        assert!(!ptr.is_null(), "ocara_runtime: OOM in __alloc_obj");
        ptr as i64
    }
}

/// Libère un bloc alloué par `__alloc_obj` (même `size`, en octets, que celle
/// passée à l'allocation — rien ne la mémorise ailleurs). Utilisée pour le frame heap d'un générateur (`emit`/
/// `message<T>`, voir docs/roadmap.d/langage-emit-iterable.md et
/// `src/lower/builder.d/message_gen.rs`) : pas de tag/header à sauter (contrairement
/// à `rc::free_object`, réservé à `__alloc_class_obj`), le pointeur libéré est
/// exactement celui retourné par `__alloc_obj`.
#[unsafe(no_mangle)]
pub extern "C" fn __free_obj(ptr: i64, size: i64) {
    if ptr == 0 || size <= 0 { return; }
    unsafe {
        let layout = Layout::from_size_align(size as usize, 8).unwrap();
        dealloc(ptr as *mut u8, layout);
    }
}

/// Layout : `[desc][rc][class_id][TAG_OBJECT][champs...]`. `class_id` reste
/// à `val - 16` (lu par `is Classe` et le dispatch) ; `desc` est le masque
/// des champs tas de la classe, une chaîne littérale `'0'`/`'1'` (un
/// caractère par champ) qui donne aussi la taille à libérer — voir
/// `crate::rc::alloc_object`.
///
/// `size == 0` (classe sans aucun champ) reste une allocation valide : une
/// classe vide a besoin d'un en-tête pour porter son identité.
#[unsafe(no_mangle)]
pub extern "C" fn __alloc_class_obj(size: i64, class_id: i64, desc: i64) -> i64 {
    if size < 0 { return 0; }
    unsafe { rc::alloc_object(size as usize, class_id, desc) }
}

/// Alloue un fat pointer (Function) avec tag TAG_FUNCTION.
/// 16 octets de données : {func_ptr: i64, env_ptr: i64}.
/// Le pointeur retourné pointe APRÈS le header de 8 octets.
#[unsafe(no_mangle)]
pub extern "C" fn __alloc_fat_ptr() -> i64 {
    unsafe { rc::alloc_block(16, 0, TAG_FUNCTION, true) }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cellule verrouillée pour les variables capturées par une closure/thread
//
// Une variable capturée par une `nameless` est "promue sur le tas" (voir
// `src/lower/expr.d/lower.rs`) : le scope extérieur et la closure partagent
// alors le même pointeur, potentiellement lu/écrit depuis plusieurs threads
// à la fois (Thread::run, workers HTTPServer) — sans ces fonctions, un accès
// concurrent était un comportement non défini. Chaque cellule embarque
// désormais son propre mutex : `__alloc_locked_cell` réserve les octets d'un
// `pthread_mutex_t` juste AVANT la valeur (même convention "header avant le
// pointeur retourné" que les tags `TAG_*` du reste du runtime), et
// `__locked_cell_get`/`__locked_cell_set` remplacent tout accès direct
// (Load/Store) à une variable capturée dans le lowering.
//
// Mutex séparé de celui de `ocara.Mutex` (mutex.rs) : plus petit (pas de
// support trylock/destroy, jamais nécessaire ici), à usage interne uniquement
// (jamais exposé au langage), pas de gestion d'exception. Réutilise le même
// module `platform` que `ocara.Mutex` (pthread_mutex sur Unix,
// CRITICAL_SECTION sur Windows, voir mutex.rs) plutôt qu'un tableau d'octets
// hardcodé par plateforme redéclaré ici séparément — c'était le cas avant
// (`[u8;40]` Linux / `[u8;64]` macOS/repli), jamais vérifié contre la vraie
// taille ABI (même risque que documenté dans mutex.rs), et de toute façon
// jamais correct sur Windows (aucune fonction `pthread_mutex_*` n'y existe,
// confirmé par échec de lien réel en cross-compilant vers
// `x86_64-pc-windows-gnu` — voir docs/roadmap.d/packaging-windows.md).
use crate::mutex::platform as cell_mutex_platform;
type CapturedCellMutex = cell_mutex_platform::RawMutex;

const CAPTURED_CELL_MUTEX_SIZE: usize = std::mem::size_of::<CapturedCellMutex>();

/// Alloue une cellule de capture verrouillée, comptée :
/// `[en-tête TAG_CELL][valeur: i64][mutex]`, pointeur sur la valeur.
/// `counted != 0` : la valeur est une référence comptée, relâchée avec la
/// cellule (voir `crate::rc`).
#[unsafe(no_mangle)]
pub extern "C" fn __alloc_locked_cell(counted: i64) -> i64 {
    unsafe {
        let cell = rc::alloc_block(8 + CAPTURED_CELL_MUTEX_SIZE, (counted != 0) as i64, crate::typecheck::TAG_CELL, true);
        cell_mutex_platform::init(cell_mutex(cell));
        cell
    }
}

pub(crate) const CELL_PAYLOAD: usize = 8 + CAPTURED_CELL_MUTEX_SIZE;

fn cell_mutex(cell_ptr: i64) -> *mut CapturedCellMutex {
    (cell_ptr + 8) as *mut CapturedCellMutex
}

pub(crate) unsafe fn free_cell(cell_ptr: i64) {
    unsafe {
        cell_mutex_platform::destroy(cell_mutex(cell_ptr));
        rc::free_block(cell_ptr, CELL_PAYLOAD);
    }
}

/// Lit la valeur d'une cellule de capture sous verrou.
#[unsafe(no_mangle)]
pub extern "C" fn __locked_cell_get(cell_ptr: i64) -> i64 {
    unsafe {
        cell_mutex_platform::lock(cell_mutex(cell_ptr));
        let val = *(cell_ptr as *const i64);
        cell_mutex_platform::unlock(cell_mutex(cell_ptr));
        val
    }
}

/// Écrit la valeur d'une cellule de capture sous verrou.
#[unsafe(no_mangle)]
pub extern "C" fn __locked_cell_set(cell_ptr: i64, val: i64) {
    unsafe {
        cell_mutex_platform::lock(cell_mutex(cell_ptr));
        *(cell_ptr as *mut i64) = val;
        cell_mutex_platform::unlock(cell_mutex(cell_ptr));
    }
}

/// Cellule d'une variable de générateur capturée par une closure : celle
/// déjà rangée dans le frame (`existing`), sinon une cellule neuve —
/// créée au premier accès, partagée ensuite par le corps du générateur et
/// ses closures (voir docs/roadmap.d/langage-emit-iterable.md). Sa valeur
/// initiale est celle du champ de la variable (`initial` : l'argument d'un
/// paramètre, nulle pour une locale), retenue par la cellule.
#[unsafe(no_mangle)]
pub extern "C" fn __gen_cell(existing: i64, counted: i64, initial: i64) -> i64 {
    if existing != 0 {
        return existing;
    }
    let cell = __alloc_locked_cell(counted);
    if counted != 0 {
        unsafe { rc::retain(initial) }
    }
    __locked_cell_set(cell, initial);
    cell
}

/// Frame d'un générateur : bloc compté de `size` octets, détruit par
/// `drop_fn` (`<générateur>__drop`, qui rend ses champs comptés puis appelle
/// `__free_gen`).
#[unsafe(no_mangle)]
pub extern "C" fn __alloc_gen(size: i64, drop_fn: i64) -> i64 {
    unsafe { rc::alloc_block(size.max(8) as usize, drop_fn, crate::typecheck::TAG_GEN, true) }
}

#[unsafe(no_mangle)]
pub extern "C" fn __free_gen(frame: i64, size: i64) {
    unsafe { rc::free_block(frame, size.max(8) as usize) }
}

/// Environnement d'une closure : `n_fields` champs dont les `n_caps`
/// premiers sont des cellules capturées (comptées), les suivants des valeurs
/// par défaut de paramètres.
#[unsafe(no_mangle)]
pub extern "C" fn __alloc_env(n_caps: i64, n_fields: i64) -> i64 {
    let n_fields = n_fields.max(1);
    unsafe { rc::alloc_block(n_fields as usize * 8, n_caps | (n_fields << 32), crate::typecheck::TAG_ENV, true) }
}

// ─────────────────────────────────────────────────────────────────────────────
// try / on / fail — mécanisme setjmp/longjmp
//
// On utilise des extern "C" vers setjmp/longjmp de libc (toujours disponibles
// dans le binaire final car la libc est systématiquement liée).
//
// Contrainte : setjmp doit être appelé dans une frame qui reste vivante.
// C'est le cas ici : __ocara_try_exec appelle setjmp dans sa propre frame,
// puis appelle body_fn — la frame de __ocara_try_exec est encore sur la pile
// quand body_fn s'exécute (y compris les fonctions qu'elle appelle).
// Quand longjmp est déclenché depuis __ocara_fail, le flux de contrôle
// reprend à l'intérieur de __ocara_try_exec, qui est toujours vivant.
// ─────────────────────────────────────────────────────────────────────────────

// jmp_buf sur x86-64 Linux = 25 × u64 = 200 octets
#[repr(C, align(8))]
struct JmpBuf([u64; 25]);

unsafe extern "C" {
    /// `_setjmp` : sans sauvegarde du masque de signaux (`setjmp` de glibc
    /// faisait deux `rt_sigprocmask` par exception).
    #[allow(improper_ctypes)]
    #[cfg_attr(not(windows), link_name = "_setjmp")]
    fn setjmp(env: *mut JmpBuf) -> i32;
    #[allow(improper_ctypes)]
    fn longjmp(env: *mut JmpBuf, val: i32) -> !;
}

// État de la pile try (par thread, profondeur max = 64)
struct TryFrame {
    env:        JmpBuf,
    error_val:  i64,
    error_type: i64,
    /// Profondeur de la pile de déroulement (`rc::unwind_depth`) à l'entrée.
    unwind:     usize,
    /// Nombre de ressources ouvertes (`rc::resource_depth`) à l'entrée.
    resources:  usize,
}

const MAX_TRY_DEPTH: usize = 64;

use std::cell::Cell;
use std::cell::UnsafeCell;

struct TryStack {
    frames: UnsafeCell<[TryFrame; MAX_TRY_DEPTH]>,
    depth:  Cell<usize>,
}

// Safety: single-threaded per thread (thread_local!)
unsafe impl Sync for TryStack {}

thread_local! {
    static TRY_STACK: TryStack = TryStack {
        frames: UnsafeCell::new(unsafe {
            // Initialisation par zero-fill (JmpBuf et i64 sont tous POD)
            std::mem::zeroed()
        }),
        depth: Cell::new(0),
    };
}

/// Code de sortie d'un corps `try` ou d'un gestionnaire `on` (fonctions
/// séparées, voir `lower_try`) : fin normale, ou `return`/`break`/`continue`
/// à exécuter dans la fonction englobante. La valeur d'un `return` passe par
/// `__ocara_try_set_return`/`__ocara_try_take_return`.
const TRY_NORMAL: i64 = 0;

thread_local! {
    static TRY_RETURN: Cell<i64> = const { Cell::new(0) };
}

#[unsafe(no_mangle)]
pub extern "C" fn __ocara_try_set_return(value: i64) {
    TRY_RETURN.with(|v| v.set(value));
}

#[unsafe(no_mangle)]
pub extern "C" fn __ocara_try_take_return() -> i64 {
    TRY_RETURN.with(|v| v.replace(0))
}

/// Pousse un nouveau `TryFrame` sur `TRY_STACK` et retourne un pointeur vers
/// lui (adresse STABLE — voir la doc de `TryStack`/`frames`, un tableau fixe
/// thread-local, jamais réalloué) directement utilisable par l'appelant pour
/// un appel `setjmp` RAW (voir `crate::lower::builder::message_gen` côté
/// compilateur) : contrairement à `__ocara_try_exec`, aucun `setjmp` n'est
/// fait ICI — cette fonction ne fait QUE réserver le frame, le `setjmp` doit
/// être appelé PAR L'APPELANT, directement dans SA PROPRE frame native (voir
/// la doc de `JmpBuf` plus haut : le frame appelant setjmp doit rester
/// vivant jusqu'au `longjmp` correspondant). C'est précisément ce qui permet
/// à un générateur (`emit`/`message<T>`) de rejouer un `try` à chaque
/// reprise — sa fonction `__resume` est un nouvel appel natif à chaque fois,
/// donc SA PROPRE frame doit appeler `setjmp` elle-même.
///
/// `frame_ptr` (retourné en i64) pointe vers un `TryFrame` : offset 0 =
/// `env` (JmpBuf, 200 octets — à passer tel quel à `setjmp`), offset 200 =
/// `error_val`, offset 208 = `error_type` (voir `Inst::GetField` avec ces
/// offsets bruts dans le lowering, pas de fonction runtime dédiée pour les
/// lire — même patron que `class_id` dans `__alloc_class_obj`).
#[unsafe(no_mangle)]
pub extern "C" fn __ocara_try_enter() -> i64 {
    TRY_STACK.with(|stack| {
        let depth = stack.depth.get();
        if depth >= MAX_TRY_DEPTH {
            std::process::abort();
        }
        let frame_ptr: *mut TryFrame = unsafe {
            let arr = &mut *stack.frames.get();
            &mut arr[depth]
        };
        unsafe {
            (*frame_ptr).error_val  = 0;
            (*frame_ptr).error_type = 0;
            (*frame_ptr).unwind = rc::unwind_depth();
            (*frame_ptr).resources = rc::resource_depth();
        }
        stack.depth.set(depth + 1);
        frame_ptr as i64
    })
}

/// Dépile le `TryFrame` le plus récemment poussé par `__ocara_try_enter` —
/// appelée après une sortie normale du corps `try`, après un handler qui a
/// fini de traiter l'exception, OU juste avant qu'un `emit` (générateur)
/// suspende en quittant un `try` encore actif (pour rejouer `__ocara_try_enter`
/// + `setjmp` à la prochaine reprise). Symétrique de `__ocara_try_enter`,
/// même discipline LIFO que `__ocara_try_exec`.
#[unsafe(no_mangle)]
pub extern "C" fn __ocara_try_exit() {
    TRY_STACK.with(|stack| {
        let depth = stack.depth.get();
        if depth > 0 {
            stack.depth.set(depth - 1);
        }
    });
}

/// Exécute un bloc try/on et retourne le code de sortie du corps, ou du
/// gestionnaire si une exception a été levée (voir `TRY_NORMAL`).
#[unsafe(no_mangle)]
pub extern "C" fn __ocara_try_exec(body_fn: i64, handler_fn: i64) -> i64 {
    TRY_STACK.with(|stack| {
        let depth = stack.depth.get();
        if depth >= MAX_TRY_DEPTH {
            std::process::abort();
        }

        // Obtenir un pointeur vers le frame courant
        let frame_ptr: *mut TryFrame = unsafe {
            let arr = &mut *stack.frames.get();
            &mut arr[depth]
        };

        // Initialiser le frame
        unsafe {
            (*frame_ptr).error_val  = 0;
            (*frame_ptr).error_type = 0;
            (*frame_ptr).unwind = rc::unwind_depth();
            (*frame_ptr).resources = rc::resource_depth();
        }

        // Pousser la nouvelle profondeur
        stack.depth.set(depth + 1);

        // SETJMP — sauvegarde la frame de __ocara_try_exec
        let env_ptr: *mut JmpBuf = unsafe { &mut (*frame_ptr).env };
        let ret = unsafe { setjmp(env_ptr) };

        if ret == 0 {
            // Exécution normale du corps try
            let code = unsafe {
                let body: unsafe extern "C" fn() -> i64 =
                    std::mem::transmute(body_fn as usize);
                body()
            };
            // Sortie normale : dépiler
            stack.depth.set(depth);
            code
        } else {
            // longjmp déclenché : récupérer error_val et error_type
            let (ev, et) = unsafe {
                ((*frame_ptr).error_val, (*frame_ptr).error_type)
            };
            // Dépiler avant d'appeler le handler
            stack.depth.set(depth);
            // Appeler le gestionnaire
            let mut held = [0i64; 2];
            hold_raised(&mut held, ev, et);
            let code = unsafe {
                let handler: unsafe extern "C" fn(i64, i64) -> i64 =
                    std::mem::transmute(handler_fn as usize);
                handler(ev, et)
            };
            release_held(&mut held);
            code
        }
    })
}

/// La valeur levée et son nom de type appartiennent à la frame `try` (`raise`
/// les transfère, voir `lower_raise`) : relâchés une fois le gestionnaire
/// revenu. Un gestionnaire qui relève sort par `longjmp` et transfère la
/// même référence plus haut.
/// Valeur levée et nom de type tenus pendant l'appel du gestionnaire : inscrits
/// sur la pile de déroulement (`rc::__rc_unwind_push`, `held` vit dans la frame
/// du trampoline), ils sont rendus par `unwind_to` si le gestionnaire relève
/// (`longjmp` qui saute cette frame) — sinon l'exception rattrapée `e`
/// fuyait (`raise` dans un `on`).
fn hold_raised(held: &mut [i64; 2], ev: i64, et: i64) {
    rc::__rc_unwind_push(held.as_mut_ptr() as i64, 2);
    *held = [ev, et];
}

/// Gestionnaire revenu normalement : retire l'inscription, puis rend.
fn release_held(held: &mut [i64; 2]) {
    rc::__rc_unwind_pop();
    let [ev, et] = std::mem::take(held);
    release_raised(ev, et);
}

fn release_raised(ev: i64, et: i64) {
    unsafe {
        rc::release(ev);
        rc::release(et);
    }
}

/// Version dynamique avec pointeur vers tableau de captures.
/// Le body reçoit un *const i64 pointant vers le début du tableau.
/// Retourne le code de sortie, comme `__ocara_try_exec`.
#[unsafe(no_mangle)]
pub extern "C" fn __ocara_try_exec_with_captures(
    body_fn: i64,
    handler_fn: i64,
    captures_ptr: *const i64,
) -> i64 {
    TRY_STACK.with(|stack| {
        let depth = stack.depth.get();
        if depth >= MAX_TRY_DEPTH {
            std::process::abort();
        }

        let frame_ptr: *mut TryFrame = unsafe {
            let arr = &mut *stack.frames.get();
            &mut arr[depth]
        };

        unsafe {
            (*frame_ptr).error_val  = 0;
            (*frame_ptr).error_type = 0;
            (*frame_ptr).unwind = rc::unwind_depth();
            (*frame_ptr).resources = rc::resource_depth();
        }

        stack.depth.set(depth + 1);

        let env_ptr: *mut JmpBuf = unsafe { &mut (*frame_ptr).env };
        let ret = unsafe { setjmp(env_ptr) };

        if ret == 0 {
            // Le body reçoit le pointeur vers les captures
            let code = unsafe {
                let body: unsafe extern "C" fn(*const i64) -> i64 =
                    std::mem::transmute(body_fn as usize);
                body(captures_ptr)
            };
            stack.depth.set(depth);
            code
        } else {
            let (ev, et) = unsafe {
                ((*frame_ptr).error_val, (*frame_ptr).error_type)
            };
            stack.depth.set(depth);
            // Le gestionnaire reçoit ici le MÊME pointeur de captures que le
            // corps (voir `lower_try` / `src/lower/stmt.d/statements.d/exceptions.rs`) :
            // corps et gestionnaire sont deux fonctions IR distinctes qui
            // doivent partager EXACTEMENT le même tableau de cellules
            // verrouillées pour qu'une variable modifiée dans l'un (le plus
            // souvent le corps) reste visible dans l'autre et après le try.
            // `handler_fn` n'a ce 3ᵉ paramètre que si le lowering a détecté
            // des captures (corps ET/OU gestionnaires) — sinon il garde la
            // signature à 2 arguments appelée par `__ocara_try_exec`.
            let mut held = [0i64; 2];
            hold_raised(&mut held, ev, et);
            let code = unsafe {
                let handler: unsafe extern "C" fn(i64, i64, *const i64) -> i64 =
                    std::mem::transmute(handler_fn as usize);
                handler(ev, et, captures_ptr)
            };
            release_held(&mut held);
            code
        }
    })
}

/// Exécute une closure Ocara (fat pointer déjà déballé en `func_ptr`/`env_ptr`,
/// signature `fn(env_ptr: i64) -> i64`) sous protection d'une frame try dédiée,
/// SANS gestionnaire attaché : contrairement à `__ocara_try_exec`, une
/// exception interceptée ici n'est pas traitée mais renvoyée à l'appelant
/// Rust (`Err((error_val, error_type))`), qui peut alors faire un nettoyage
/// (déverrouiller un mutex, fermer une ressource...) avant de relancer
/// l'exception lui-même via `__ocara_fail` — c'est le mécanisme derrière
/// `Mutex::withLock` (voir runtime/src/mutex.rs). Un retour normal donne
/// `Ok(valeur_de_retour_de_la_closure)`.
pub(crate) fn run_closure_catching(func_ptr: i64, env_ptr: i64) -> Result<i64, (i64, i64)> {
    TRY_STACK.with(|stack| {
        let depth = stack.depth.get();
        if depth >= MAX_TRY_DEPTH {
            std::process::abort();
        }

        let frame_ptr: *mut TryFrame = unsafe {
            let arr = &mut *stack.frames.get();
            &mut arr[depth]
        };

        unsafe {
            (*frame_ptr).error_val  = 0;
            (*frame_ptr).error_type = 0;
            (*frame_ptr).unwind = rc::unwind_depth();
            (*frame_ptr).resources = rc::resource_depth();
        }

        stack.depth.set(depth + 1);

        let jmp_env: *mut JmpBuf = unsafe { &mut (*frame_ptr).env };
        let ret = unsafe { setjmp(jmp_env) };

        if ret == 0 {
            let closure: unsafe extern "C" fn(i64) -> i64 =
                unsafe { std::mem::transmute(func_ptr as usize) };
            let value = unsafe { closure(env_ptr) };
            stack.depth.set(depth);
            Ok(value)
        } else {
            let (ev, et) = unsafe {
                ((*frame_ptr).error_val, (*frame_ptr).error_type)
            };
            stack.depth.set(depth);
            Err((ev, et))
        }
    })
}

/// Comme `run_closure_catching`, mais pour une closure Ocara qui reçoit UN
/// argument supplémentaire en plus de `env_ptr` (signature `fn(env_ptr: i64,
/// arg1: i64) -> i64`) — nécessaire pour un patron "ouvrir une ressource,
/// exécuter la closure AVEC la ressource fraîchement créée en paramètre,
/// refermer systématiquement" (`SQLite::withOpen`/`MySQL::withConnect`, voir
/// runtime/src/sqlite.rs/mysql.rs) : contrairement à `Mutex::withLock`, où le
/// mutex existe déjà et est seulement CAPTURÉ par la closure (`nameless():
/// void {...}`, zéro paramètre), ici la ressource n'existe pas encore avant
/// l'appel — elle doit être PASSÉE (`nameless(db:SQLite): void {...}`, un
/// paramètre) plutôt que capturée. Duplique volontairement la mécanique
/// `TRY_STACK`/`setjmp` de `run_closure_catching` plutôt que de la
/// refactoriser : ajout pur à un nouvel endroit, risque minimal sur le
/// chemin déjà testé (voir docs/roadmap.d/exceptions-setjmp-longjmp-dette.md).
pub(crate) fn run_closure_catching_with_arg(func_ptr: i64, env_ptr: i64, arg1: i64) -> Result<i64, (i64, i64)> {
    TRY_STACK.with(|stack| {
        let depth = stack.depth.get();
        if depth >= MAX_TRY_DEPTH {
            std::process::abort();
        }

        let frame_ptr: *mut TryFrame = unsafe {
            let arr = &mut *stack.frames.get();
            &mut arr[depth]
        };

        unsafe {
            (*frame_ptr).error_val  = 0;
            (*frame_ptr).error_type = 0;
            (*frame_ptr).unwind = rc::unwind_depth();
            (*frame_ptr).resources = rc::resource_depth();
        }

        stack.depth.set(depth + 1);

        let jmp_env: *mut JmpBuf = unsafe { &mut (*frame_ptr).env };
        let ret = unsafe { setjmp(jmp_env) };

        if ret == 0 {
            let closure: unsafe extern "C" fn(i64, i64) -> i64 =
                unsafe { std::mem::transmute(func_ptr as usize) };
            let value = unsafe { closure(env_ptr, arg1) };
            stack.depth.set(depth);
            Ok(value)
        } else {
            let (ev, et) = unsafe {
                ((*frame_ptr).error_val, (*frame_ptr).error_type)
            };
            stack.depth.set(depth);
            Err((ev, et))
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn __ocara_fail(val: i64, type_name: i64) {
    let jumped = TRY_STACK.with(|stack| {
        let depth = stack.depth.get();
        if depth == 0 { return false; }

        let frame_ptr: *mut TryFrame = unsafe {
            let arr = &mut *stack.frames.get();
            &mut arr[depth - 1]
        };

        unsafe {
            (*frame_ptr).error_val  = val;
            (*frame_ptr).error_type = type_name;
            rc::close_resources_to((*frame_ptr).resources);
            rc::unwind_to((*frame_ptr).unwind);
            let env_ptr: *mut JmpBuf = &mut (*frame_ptr).env;
            longjmp(env_ptr, 1);
        }
    });

    if !jumped {
        // Aucun try actif : message d'erreur + exit
        // `type_name` porte la chaîne d'ancêtres de la classe levée, elle-même
        // en premier (voir IrModule::ancestor_chain/lower_raise) — n'afficher
        // que ce premier maillon (le nom réellement levé), pas toute la
        // chaîne de parents.
        let type_str = unsafe { ptr_to_str(type_name) }
            .split('|').next().unwrap_or("").to_string();
        let max_width = 46;
        let char_count = type_str.chars().count();
        
        // Tronquer et padder manuellement pour gérer correctement les caractères Unicode
        let display_val = if char_count > max_width {
            let truncated: String = type_str.chars().take(max_width - 3).collect();
            let padding = " ".repeat(max_width - (max_width - 3) - 3);  // padding pour "..."
            format!("{}...{}", truncated, padding)
        } else {
            let padding = " ".repeat(max_width - char_count);
            format!("{}{}", type_str, padding)
        };
        
        let msg = format!(
            "\x1b[31m╔═════════════════════════════════════════════════════════════════╗\n\
             ║ UNHANDLED EXCEPTION                                             ║\n\
             ╠═════════════════════════════════════════════════════════════════╣\n\
             ║ Exception raised: {}║\n\
             ║                                                                 ║\n\
             ║ No try/on block found to catch this exception.                  ║\n\
             ║                                                                 ║\n\
             ║ Solutions:                                                      ║\n\
             ║  • Wrap the code in a try/on block to catch exceptions          ║\n\
             ║  • Run unit tests with 'ocaraunit' instead of direct execution  ║\n\
             ╚═════════════════════════════════════════════════════════════════╝\x1b[0m\n",
            display_val
        );
        write_stderr_raw(msg.as_bytes());
        // Le message réel de l'exception (ex. l'assertion `UnitTest` en échec,
        // remontée telle quelle par ocaraunit) — jamais affiché jusqu'ici.
        if let Some(message) = unsafe { crate::exception::raised_value_message(val) } {
            write_stderr_raw(format!("\x1b[31mMessage: {}\x1b[0m\n", message).as_bytes());
        }
        // `std::process::exit()` déclenche `__cxa_finalize`, qui exécute les
        // destructeurs de TOUTES les globales C++/Rust du processus — correct
        // sur desktop (le processus Ocara EST le processus entier), mais fatal
        // sur Android : le programme Ocara ne tourne que dans un THREAD d'un
        // processus d'app partagé avec ART et les bibliothèques système déjà
        // chargées (ex. libhwui), dont les globales (mutex ART, pools de
        // threads de rendu...) ne sont jamais censées être détruites en cours
        // de vie d'une app — leur destruction est détectée comme invalide et
        // transformée en `abort()` fatal par ART/Bionic. Confirmé par
        // reproduction sur un appareil réel (voir
        // docs/roadmap.d/runtime-android-exit-unsafe.md) : `art::Mutex::~Mutex()`
        // et `android::uirenderer::CommonPool::~CommonPool()` observés comme
        // point de plantage selon quelle bibliothèque système est détruite en
        // premier. `libc::_exit()` (l'appel système brut, PAS `exit()`) saute
        // entièrement `__cxa_finalize`/les destructeurs globaux — le
        // processus termine immédiatement, sans toucher aux internes d'ART.
        #[cfg(target_os = "android")]
        unsafe { libc::_exit(1); }
        #[cfg(not(target_os = "android"))]
        std::process::exit(1);
    }
}

/// `stored` porte la chaîne d'ancêtres de la classe réellement levée (elle-
/// même incluse), du plus spécifique au plus général, jointe par `|` (ex.
/// `"FileNotFound|FileException|Exception"` — voir IrModule::ancestor_chain,
/// lower_raise). Un filtre sur une classe PARENTE doit attraper une sous-
/// classe : on cherche `filter` comme un des maillons de la chaîne, pas une
/// égalité stricte avec la valeur entière de `stored`. Reste compatible avec un `stored`
/// à un seul maillon (pas de `|`) : le split ne renvoie alors que lui-même.
#[unsafe(no_mangle)]
pub extern "C" fn __ocara_type_matches(stored: i64, filter: i64) -> i64 {
    if filter == 0 { return 1; } // pas de filtre → accepte tout
    if stored == 0 { return 0; } // pas de type stocké → ne correspond pas
    unsafe {
        let s = ptr_to_str(stored);
        let f = ptr_to_str(filter);
        if s.split('|').any(|link| link == f) { 1 } else { 0 }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Déballage des scalaires boxés
// ─────────────────────────────────────────────────────────────────────────────

/// Déboxe un float précédemment boxé par `__box_float`.
/// Entrée : `ptr | 1` (tagged heap pointer).
/// Sortie : la valeur f64 originale.
#[unsafe(no_mangle)]
pub extern "C" fn __unbox_float(tagged: i64) -> f64 {
    let ptr = (tagged & !1) as *const f64;
    unsafe { *ptr }
}

/// Déboxe un bool précédemment boxé par `__box_bool`.
/// Entrée : `ptr | 2` (tagged heap pointer).
/// Sortie : 0 ou 1 comme i64.
#[unsafe(no_mangle)]
pub extern "C" fn __unbox_bool(tagged: i64) -> i64 {
    i64::from(unsafe { unbox_bool(tagged) })
}

/// Déboxe un int précédemment boxé par `__box_int_for_mixed` (en ligne ou
/// `ptr | 3`) ; sortie : la valeur i64 originale.
#[unsafe(no_mangle)]
pub extern "C" fn __unbox_int(tagged: i64) -> i64 {
    unsafe { unbox_int(tagged) }
}

// ─────────────────────────────────────────────────────────────────────────────
// Operateurs de comparaison stricte avec verification de type
// ─────────────────────────────────────────────────────────────────────────────
//
// NOTE : L'implementation actuelle est limitee car les types primitifs
// (int, float non-boxe, bool) ne portent pas d'information de type au runtime.
// Les floats sont bitcastes en i64, donc indistinguables des grands entiers.
//
// Solution actuelle : comparaison basee sur les tags des types heap (string,
// array, map, object, function). Pour les types primitifs, on compare les
// valeurs brutes. Cela signifie qu'un int et un float avec la meme representation
// binaire seront consideres comme egaux (limitation acceptee).
// ─────────────────────────────────────────────────────────────────────────────

const PTR_THRESHOLD: i64 = 65536;
const MAX_USERSPACE_ADDR: i64 = 0x800000000000; // 128 TB, limite typique Linux

/// Determine le type d'une valeur pour les comparaisons strictes.
/// Retourne un code de type :
///   0 = null
///   1 = primitif (int/float/bool non distinguables au runtime)
///   4 = string
///   5 = array
///   6 = map
///   7 = object
///   8 = function
pub(crate) fn get_value_type(val: i64) -> i32 {
    if val == 0 {
        return 0; // null
    }
    
    // Petits entiers/bool : certainement pas des pointeurs
    if val > 0 && val < PTR_THRESHOLD {
        return 1; // primitif
    }
    
    // Si val est negatif ou >= MAX_USERSPACE_ADDR, ce n'est pas un pointeur heap valide
    // (probablement un float bitcaste ou un grand entier)
    if val < 0 || val >= MAX_USERSPACE_ADDR {
        return 1; // primitif
    }
    
    // Si val est dans la plage des pointeurs heap et aligne sur 8 octets,
    // on peut verifier les tags
    if val >= PTR_THRESHOLD && val < MAX_USERSPACE_ADDR && (val & 7) == 0 {
        // string
        if __is_string(val) != 0 {
            return 4;
        }
        // array
        if __is_array(val) != 0 {
            return 5;
        }
        // map
        if __is_map(val) != 0 {
            return 6;
        }
        // object
        if __is_object(val) != 0 {
            return 7;
        }
        // function
        if __is_function(val) != 0 {
            return 8;
        }
    }
    
    // Par defaut : primitif (int/float/bool non distinguables)
    1
}

/// Compare deux valeurs "primitif" (`get_value_type == 1` : int/float/bool,
/// brutes ou boxées) — partagé par `__cmp_eq_strict`/`ne`/`lt`/`gt`/`le`/`ge`.
/// Décide flottant vs entier comme `__dyn_add` (`is_float_box` sur CHAQUE
/// opérande) : une comparaison purement entière reste une comparaison i64
/// exacte (jamais de perte de précision en passant par un f64 — significatif
/// au-delà de 2^53), et un `int`/`bool` boxé est déballé avant de comparer —
/// comparer l'ADRESSE boxée telle quelle (l'ancien comportement) donnerait un
/// résultat sans rapport avec la valeur réelle. Corrige au passage, pour
/// int ET float, la limitation qui existait avant l'introduction du boxing
/// int (comparaison sur les bits bruts, correcte seulement pour deux int).
#[inline]
fn cmp_primitive(lhs: i64, rhs: i64, int_cmp: fn(i64, i64) -> bool, float_cmp: fn(f64, f64) -> bool) -> bool {
    if is_float_box(lhs) || is_float_box(rhs) {
        float_cmp(unbox_numeric_f64(lhs), unbox_numeric_f64(rhs))
    } else {
        int_cmp(unbox_numeric_i64(lhs), unbox_numeric_i64(rhs))
    }
}

/// `equal` : retourne 1 si meme type ET meme valeur, 0 sinon.
/// N'est appele que lorsque sema n'a pas pu verifier statiquement les types
/// (au moins un operande `mixed`) ; sinon le compilateur emet une comparaison
/// directe (CmpEq) car les types sont deja garantis compatibles.
#[unsafe(no_mangle)]
pub extern "C" fn __cmp_eq_strict(lhs: i64, rhs: i64) -> i64 {
    let lhs_type = get_value_type(lhs);
    let rhs_type = get_value_type(rhs);

    // Types differents -> false
    if lhs_type != rhs_type {
        return 0;
    }

    // Null
    if lhs_type == 0 {
        return 1; // null === null
    }

    // Types primitifs (int/float/bool, bruts ou boxés) : voir cmp_primitive.
    // Un bool logé dans un `mixed` est toujours boxé : `0 equal false` est faux.
    if lhs_type == 1 {
        if is_bool_box(lhs) != is_bool_box(rhs) {
            return 0;
        }
        return if cmp_primitive(lhs, rhs, |a, b| a == b, |a, b| a == b) { 1 } else { 0 };
    }

    // Strings : comparer le contenu
    if lhs_type == 4 {
        return if unsafe { ptr_to_str(lhs) == ptr_to_str(rhs) } { 1 } else { 0 };
    }

    // Autres types heap : comparaison de pointeurs
    if lhs == rhs { 1 } else { 0 }
}

/// `not equal` : retourne 1 si types differents OU valeurs differentes, 0 sinon.
#[unsafe(no_mangle)]
pub extern "C" fn __cmp_ne_strict(lhs: i64, rhs: i64) -> i64 {
    if __cmp_eq_strict(lhs, rhs) != 0 { 0 } else { 1 }
}

/// `smaller` : retourne 1 si meme type ET lhs < rhs, 0 sinon. Voir `cmp_primitive`.
#[unsafe(no_mangle)]
pub extern "C" fn __cmp_lt_strict(lhs: i64, rhs: i64) -> i64 {
    let lhs_type = get_value_type(lhs);
    let rhs_type = get_value_type(rhs);

    if lhs_type != rhs_type {
        return 0;
    }
    if lhs_type == 1 {
        return if cmp_primitive(lhs, rhs, |a, b| a < b, |a, b| a < b) { 1 } else { 0 };
    }
    0
}

/// `greater` : retourne 1 si meme type ET lhs > rhs, 0 sinon. Voir `cmp_primitive`.
#[unsafe(no_mangle)]
pub extern "C" fn __cmp_gt_strict(lhs: i64, rhs: i64) -> i64 {
    let lhs_type = get_value_type(lhs);
    let rhs_type = get_value_type(rhs);

    if lhs_type != rhs_type {
        return 0;
    }
    if lhs_type == 1 {
        return if cmp_primitive(lhs, rhs, |a, b| a > b, |a, b| a > b) { 1 } else { 0 };
    }
    0
}

/// `smaller or equal` : retourne 1 si meme type ET lhs <= rhs, 0 sinon. Voir `cmp_primitive`.
#[unsafe(no_mangle)]
pub extern "C" fn __cmp_le_strict(lhs: i64, rhs: i64) -> i64 {
    let lhs_type = get_value_type(lhs);
    let rhs_type = get_value_type(rhs);

    // Types differents -> false
    if lhs_type != rhs_type {
        return 0;
    }

    // Primitifs (int/float/bool, bruts ou boxés) : voir cmp_primitive
    if lhs_type == 1 {
        return if cmp_primitive(lhs, rhs, |a, b| a <= b, |a, b| a <= b) { 1 } else { 0 };
    }

    // Autres types non comparables
    0
}

/// `greater or equal` : retourne 1 si meme type ET lhs >= rhs, 0 sinon. Voir `cmp_primitive`.
#[unsafe(no_mangle)]
pub extern "C" fn __cmp_ge_strict(lhs: i64, rhs: i64) -> i64 {
    let lhs_type = get_value_type(lhs);
    let rhs_type = get_value_type(rhs);

    // Types differents -> false
    if lhs_type != rhs_type {
        return 0;
    }

    // Primitifs (int/float/bool, bruts ou boxés) : voir cmp_primitive
    if lhs_type == 1 {
        return if cmp_primitive(lhs, rhs, |a, b| a >= b, |a, b| a >= b) { 1 } else { 0 };
    }

    // Autres types non comparables
    0
}

// ────────────────────────────────────────────────────────────────────────────
// Arithmétique dynamique (`+`/`-`/`*`/`/`/`%`) avec un opérande `mixed`
// ────────────────────────────────────────────────────────────────────────────
// Comme pour les comparaisons strictes ci-dessus, `mixed` n'est pas taggé au
// niveau du type IR statique (toujours réduit à `Ptr`, voir `IrType::from_ast`)
// — mais un `mixed` contenant un `int` est stocké BRUT (jamais boxé,
// `box_for_any` ne boxe que float/bool), donc numériquement correct tel quel ;
// un `mixed` contenant un `float`/`bool` est boxé (`__box_float`/`__box_bool`,
// tag dans les 2 bits bas, voir plus haut) et doit être déballé avant tout
// calcul. Sans ce dispatch, le lowering traitait soit AUCUN opérande Ptr comme
// numérique (le cas de `+`, qui supposait systématiquement une concaténation
// string dès qu'un opérande est `Ptr` — donc aussi pour un `mixed` contenant un
// entier), soit ne déballait JAMAIS un `mixed` boxé (float/bool) avant `-`/`*`/
// `/`/`%`, qui opéraient alors sur le bit pattern du pointeur boxé lui-même.

/// Déballe un opérande potentiellement `mixed` en entier — un entier brut (pas
/// boxé) est déjà correct tel quel ; un float/bool boxé est reconverti ; un
/// vrai pointeur tas (string/array/map/objet, déjà mal typé dans un contexte
/// arithmétique) est laissé tel quel (comportement dégradé mais déterministe,
/// identique au bit brut déjà utilisé avant ce correctif pour ce cas).
#[inline]
fn unbox_numeric_i64(val: i64) -> i64 {
    if is_float_box(val) {
        unsafe { unbox_float(val) as i64 }
    } else if is_bool_box(val) {
        if unsafe { unbox_bool(val) } { 1 } else { 0 }
    } else if is_int_box(val) {
        unsafe { unbox_int(val) }
    } else {
        val
    }
}

/// Comme `unbox_numeric_i64`, mais pour un contexte flottant — un entier brut
/// est converti numériquement (jamais un bitcast).
#[inline]
fn unbox_numeric_f64(val: i64) -> f64 {
    if is_float_box(val) {
        unsafe { unbox_float(val) }
    } else if is_bool_box(val) {
        if unsafe { unbox_bool(val) } { 1.0 } else { 0.0 }
    } else if is_int_box(val) {
        unsafe { unbox_int(val) as f64 }
    } else {
        val as f64
    }
}

/// Vrai si `val` est un vrai objet tas (string/array/map/objet/fonction) —
/// PAS un entier brut, ni un float/bool boxé (voir `get_value_type`, déjà
/// utilisé par les comparaisons strictes : distingue fiablement, via les tags
/// réels des allocations, un pointeur tas valide d'un entier qui y ressemble).
#[inline]
fn is_heap_object(val: i64) -> bool {
    get_value_type(val) > 1
}

/// Déballe un opérande `mixed`/Ptr en entier pour l'utiliser comme opérande
/// direct de `-`/`*`/`/`/`%` (voir `unbox_numeric_i64`) — le côté à type
/// statique connu (`int`/`float`/`bool`) d'une expression n'a jamais besoin de
/// passer par cette fonction, seul un opérande réellement `Ptr` (mixed, ou
/// littéral string/array/... déjà mal typé dans ce contexte) en a besoin.
#[unsafe(no_mangle)]
pub extern "C" fn __mixed_to_int(val: i64) -> i64 {
    unbox_numeric_i64(val)
}

/// Comme `__mixed_to_int`, pour un contexte flottant.
#[unsafe(no_mangle)]
pub extern "C" fn __mixed_to_float(val: i64) -> f64 {
    unbox_numeric_f64(val)
}

/// Valeur entière d'un opérande `mixed` sans cellule : entier brut ou en
/// ligne (chemin rapide de `__dyn_*`) ; `None` pour tout le reste.
#[inline]
fn plain_int(val: i64) -> Option<i64> {
    if val < 0 || (val > 0 && val < 0x10000) {
        Some(val)
    } else if is_inline_int(val) {
        Some(unsafe { unbox_int(val) })
    } else {
        None
    }
}

/// `+` quand au moins un opérande est `Ptr` (mixed, ou un `string`/`array`/...
/// réellement connu) : décide DYNAMIQUEMENT, au lieu de supposer
/// systématiquement une concaténation string comme avant ce correctif — un
/// `mixed` contenant un nombre doit s'additionner numériquement, pas se
/// stringifier. Concaténation conservée (comportement historique inchangé)
/// dès qu'un côté est un vrai objet tas (string/array/map/objet/fonction) ;
/// sinon, addition numérique réelle (flottante si l'un des deux est un float
/// boxé, entière sinon). Retourne une valeur "mixed" auto-décrite (pointeur
/// string, entier brut, ou float boxé) — au même titre que n'importe quelle
/// autre valeur `mixed` : c'est au consommateur (affectation vers une cible
/// `int`/`float` concrète via `box_for_any`, ou un opérateur arithmétique
/// englobant via `__mixed_to_int`/`__mixed_to_float`) de la déballer si besoin.
#[unsafe(no_mangle)]
pub extern "C" fn __dyn_add(a: i64, b: i64) -> i64 {
    match (plain_int(a), plain_int(b)) {
        (Some(x), Some(y)) => box_int_if_needed(x.wrapping_add(y)),
        _ => dyn_add_other(a, b),
    }
}

/// `__dyn_add` hors entiers sans cellule, hors ligne : son prologue pesait
/// sur le chemin entier.
#[inline(never)]
fn dyn_add_other(a: i64, b: i64) -> i64 {
    if is_heap_object(a) || is_heap_object(b) {
        return unsafe { alloc_str(&(val_to_string(a) + &val_to_string(b))) };
    }
    if is_float_box(a) || is_float_box(b) {
        return __box_float((unbox_numeric_f64(a) + unbox_numeric_f64(b)).to_bits() as i64);
    }
    // Résultat entier : ce retour EST déjà une valeur "mixed" auto-décrite
    // (voir la doc ci-dessus) — rien en aval ne le reboxera, donc c'est ICI
    // qu'il faut garantir l'invariant "un int assez grand pour être ambigu
    // avec un pointeur heap est boxé" (voir `box_int_if_needed`).
    box_int_if_needed(unbox_numeric_i64(a) + unbox_numeric_i64(b))
}

/// `-`/`*`/`/` quand au moins un opérande est `Ptr` (mixed — jamais un vrai
/// `string`/`array`/`map`/objet ici : la sema rejette déjà ces combinaisons
/// pour un type concrètement connu, voir la doc de `lower::expr::lower`).
/// Contrairement à `+`, pas de possibilité de concaténation à écarter : la
/// seule question est entier vs flottant, décidée ICI dynamiquement
/// (`is_float_box` sur CHAQUE opérande) plutôt que par le type statique de
/// l'AUTRE opérande — un `mixed` contenant un float combiné à un `int` connu
/// aurait sinon été silencieusement tronqué en entier. Retourne, comme `__dyn_add`,
/// une valeur "mixed" auto-décrite (entier brut ou float boxé).
macro_rules! dyn_arith_op {
    ($name:ident, $op:tt, $int_op:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(a: i64, b: i64) -> i64 {
            #[inline(never)]
            fn other(a: i64, b: i64) -> i64 {
                if is_float_box(a) || is_float_box(b) {
                    return __box_float((unbox_numeric_f64(a) $op unbox_numeric_f64(b)).to_bits() as i64);
                }
                box_int_if_needed(unbox_numeric_i64(a).$int_op(unbox_numeric_i64(b)))
            }
            match (plain_int(a), plain_int(b)) {
                (Some(x), Some(y)) => box_int_if_needed(x.$int_op(y)),
                _ => other(a, b),
            }
        }
    };
}
dyn_arith_op!(__dyn_sub, -, wrapping_sub);
dyn_arith_op!(__dyn_mul, *, wrapping_mul);

#[unsafe(no_mangle)]
pub extern "C" fn __dyn_div(a: i64, b: i64) -> i64 {
    if let (Some(x), Some(y)) = (plain_int(a), plain_int(b)) {
        return box_int_if_needed(__int_div(x, y));
    }
    if is_float_box(a) || is_float_box(b) {
        __box_float((unbox_numeric_f64(a) / unbox_numeric_f64(b)).to_bits() as i64)
    } else {
        box_int_if_needed(__int_div(unbox_numeric_i64(a), unbox_numeric_i64(b)))
    }
}

/// Accès au membre `member` (chaîne) d'un receveur `T|null` : `Exception`
/// si le receveur vaut `null` — lire à l'adresse 0 arrêtait le programme.
#[unsafe(no_mangle)]
pub extern "C" fn __null_receiver(val: i64, member: i64) {
    if val == 0 {
        let name = unsafe { ptr_to_str(member) };
        unsafe { exception::throw_exception(&format!("Cannot access '{}' on null", name), 0, "") }
    }
}

const ERR_MATH_DIVISION_BY_ZERO: i64 = 104;

/// `a / b` entre entiers : `MathException` (104) si `b` vaut 0 ;
/// `i64::MIN / -1` boucle (`i64::MIN`), comme tout dépassement entier
/// (`+`, `-`, `*`). Remplace `sdiv`, qui arrêtait le programme (SIGILL/SIGFPE).
#[unsafe(no_mangle)]
pub extern "C" fn __int_div(a: i64, b: i64) -> i64 {
    if b == 0 {
        unsafe { exception::throw_math_exception("Division by zero", ERR_MATH_DIVISION_BY_ZERO, "Math") }
    }
    a.wrapping_div(b)
}

/// `a % b` entre entiers : `MathException` (104) si `b` vaut 0 ;
/// `i64::MIN % -1` vaut 0.
#[unsafe(no_mangle)]
pub extern "C" fn __int_mod(a: i64, b: i64) -> i64 {
    if b == 0 {
        unsafe { exception::throw_math_exception("Modulo by zero", ERR_MATH_DIVISION_BY_ZERO, "Math") }
    }
    a.wrapping_rem(b)
}

// ────────────────────────────────────────────────────────────────────────────
// ocara.JSON — Sérialisation et désérialisation JSON
// ────────────────────────────────────────────────────────────────────────────

use serde_json::Value as JsonValue;

/// JSON::encode(data, leaf_kind) → string
/// Encode un array ou map en JSON. `leaf_kind` (2e paramètre, jamais visible
/// côté langage Ocara — voir `static_leaf_shape` côté lowering) : 0 =
/// inconnu/`mixed` (comportement heuristique historique de `value_to_json`),
/// 1/2/3 = int/float/bool — le type de feuille concret d'un conteneur qui ne
/// boxe jamais ses éléments (`array<int>`, `array<bool>`...), connu de
/// façon fiable à la compilation, sans quoi un entier brut `0`/`1` est
/// indiscernable de `null`/`false` (voir `value_to_json`).
#[unsafe(no_mangle)]
pub extern "C" fn JSON_encode(data: i64, leaf_kind: i64) -> i64 {
    json_codec::encode(data, leaf_kind)
}

/// JSON::decode(json) → mixed (array ou map) ; `null` si le texte n'est pas
/// du JSON valide (voir `json_codec::decode`).
#[unsafe(no_mangle)]
pub extern "C" fn JSON_decode(json: i64) -> i64 {
    if json == 0 {
        return 0;
    }
    json_codec::decode(unsafe { ptr_to_str(json) })
}

/// JSON::pretty(json) → string
/// Formatte le JSON avec indentation
#[unsafe(no_mangle)]
pub extern "C" fn JSON_pretty(json: i64) -> i64 {
    if json == 0 {
        return unsafe { alloc_str("") };
    }
    
    let json_str = unsafe { ptr_to_str(json) };
    
    match serde_json::from_str::<JsonValue>(json_str) {
        Ok(value) => {
            let pretty = serde_json::to_string_pretty(&value).unwrap_or_else(|_| json_str.to_string());
            unsafe { alloc_str(&pretty) }
        }
        Err(_) => __rc_passthrough(json)
    }
}

/// JSON::minimize(json) → string
/// Minifie le JSON (supprime les espaces)
#[unsafe(no_mangle)]
pub extern "C" fn JSON_minimize(json: i64) -> i64 {
    if json == 0 {
        return unsafe { alloc_str("") };
    }
    
    let json_str = unsafe { ptr_to_str(json) };
    
    match serde_json::from_str::<JsonValue>(json_str) {
        Ok(value) => {
            let minimized = serde_json::to_string(&value).unwrap_or_else(|_| json_str.to_string());
            unsafe { alloc_str(&minimized) }
        }
        Err(_) => __rc_passthrough(json)
    }
}

/// Tests unitaires du runtime — `runtime/src/tests/`, un fichier par domaine
/// (voir docs/roadmap.d/qualite-tests-unitaires-critiques.md), pour ne pas
/// alourdir davantage ce fichier (déjà le plus gros du crate).
#[cfg(test)]
mod tests;
