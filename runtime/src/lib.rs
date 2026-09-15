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

use std::alloc::{alloc, alloc_zeroed, dealloc, Layout};
use crate::typecheck::{TAG_STRING_OWNED, TAG_ARRAY, TAG_MAP, TAG_OBJECT, TAG_FUNCTION,
    __is_function, __is_object, __is_map, __is_array, __is_string, read_tag};
use std::ffi::CStr;
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
/// docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md). `__object_free`
/// (n_fields recalculé, `src/lower/builder.d/class_ownership.rs`) reste lui
/// sans changement : `n_fields` provient d'une SEULE source
/// (`class_layouts`), relue identiquement à l'allocation et à la
/// libération dans une même compilation — rien à recalculer de façon
/// divergente, contrairement au cas des strings.
/// `pub` (pas `pub(crate)`) : utilisé depuis le crate séparé runtime_tauri.
pub unsafe fn alloc_str(s: &str) -> i64 {
    let bytes = s.as_bytes();
    // 8 octets longueur + 8 octets tag + données + null-terminator
    let total = 16 + bytes.len() + 1;
    let layout = Layout::from_size_align(total, 8).unwrap();
    unsafe {
        let raw = alloc(layout);
        assert!(!raw.is_null(), "ocara_runtime: OOM");
        // Longueur réelle des données (hors NUL), lue uniquement par free_str
        *(raw as *mut i64) = bytes.len() as i64;
        // Écrire le tag dans le header (offset inchangé : val - 8)
        *(raw.add(8) as *mut i64) = TAG_STRING_OWNED;
        // Copier les données de la chaîne après le header
        let data = raw.add(16);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        *data.add(bytes.len()) = 0u8;
        // Retourner le pointeur APRÈS len+tag (= pointeur vers les données) —
        // inchangé pour tout le reste du runtime (read_tag, ptr_to_str, ...).
        (raw as i64) + 16
    }
}

/// Libère une chaîne allouée par `alloc_str` — exact inverse (même layout :
/// 8 octets de longueur + 8 octets de tag + données + NUL, alignement 8).
/// PAS un ramasse-miettes général : à utiliser uniquement sur des temporaires
/// dont la durée de vie est connue et courte (ex. valeurs FFI internes à un
/// appel), jamais sur une valeur Ocara ordinaire potentiellement encore
/// référencée ailleurs — ce runtime ne fait aucun suivi de références,
/// appeler ceci sur un pointeur encore utilisé ailleurs est un use-after-free.
/// `pub` (pas `pub(crate)`) : utilisé depuis le crate séparé runtime_tauri.
pub unsafe fn free_str(val: i64) {
    if val == 0 {
        return;
    }
    unsafe {
        // Longueur réelle stockée à l'allocation (voir alloc_str) — plus
        // fiable qu'un recalcul par recherche du premier octet NUL, qui
        // sous-estimerait la taille si la string contient un NUL interne.
        let len = *((val - 16) as *const i64) as usize;
        let raw = (val - 16) as *mut u8;
        let layout = Layout::from_size_align(16 + len + 1, 8).unwrap();
        dealloc(raw, layout);
    }
}

/// Lit un pointeur i64 comme &str (null-terminated UTF-8).
/// `pub` (pas `pub(crate)`) : utilisé depuis le crate séparé runtime_tauri.
pub unsafe fn ptr_to_str<'a>(val: i64) -> &'a str {
    if val == 0 {
        return "";
    }
    unsafe {
        let cstr = CStr::from_ptr(val as *const i8);
        cstr.to_str().unwrap_or("")
    }
}

// ─── Tagged-pointer boxing pour les valeurs `any` ───────────────────────────
// Les pointeurs heap sont alignés sur 8 octets → les 3 bits bas sont 0.
// On encode le type dans les 2 bits bas :
//   bits 1:0 = 00  → string ou objet normal (is_ptr)
//   bits 1:0 = 01  → float boxé  (__box_float)
//   bits 1:0 = 10  → bool boxé   (__box_bool)
//   bits 1:0 = 11  → int boxé   (__box_int_for_mixed) — voir plus bas

/// Retourne true si val est un pointeur string/objet (bits bas == 00).
#[inline]
fn is_ptr(val: i64) -> bool {
    val >= 0x10000 && (val & 3) == 0
}

#[inline]
fn is_float_box(val: i64) -> bool {
    val >= 0x10000 && (val & 3) == 1
}

#[inline]
fn is_bool_box(val: i64) -> bool {
    val >= 0x10000 && (val & 3) == 2
}

/// Voir `box_int_if_needed` : un `int` logé dans un `mixed` n'est boxé QUE
/// s'il est assez grand pour être confondu avec un pointeur heap (au-delà de
/// ce seuil, un pointeur réel ET un entier ordinaire sont indiscernables sans
/// boxing — voir docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md) —
/// un petit entier reste donc brut, jamais alloué.
#[inline]
fn is_int_box(val: i64) -> bool {
    val >= 0x10000 && (val & 3) == 3
}

#[inline]
unsafe fn unbox_float(val: i64) -> f64 {
    unsafe { *((val & !3) as *const f64) }
}

#[inline]
unsafe fn unbox_bool(val: i64) -> bool {
    unsafe { *((val & !3) as *const i64) != 0 }
}

#[inline]
unsafe fn unbox_int(val: i64) -> i64 {
    unsafe { *((val & !3) as *const i64) }
}

/// Boxe `n` uniquement s'il est assez grand pour être confondu avec un
/// pointeur heap valide par `read_tag`/`get_value_type` (`val >= 0x10000`,
/// le même seuil que `PTR_THRESHOLD` ailleurs dans le runtime) — un petit
/// entier reste brut (comme avant ce correctif), pas de coût d'allocation
/// pour le cas de loin le plus fréquent. Un entier NÉGATIF n'est jamais
/// ambigu avec un pointeur (toujours < 0x10000 en comparaison signée) et
/// reste donc toujours brut, quelle que soit sa magnitude.
///
/// Contrairement à `float`/`bool` (toujours boxés dans un `mixed`, sans
/// condition), cette fonction est LE point d'entrée unique qui décide, au
/// runtime, si un `int` donné a besoin d'être boxé — appelée à chaque
/// endroit qui logeait auparavant un `int` brut dans un `mixed` (affectation,
/// argument, littéral `array`/`map`, résultat arithmétique dynamique...).
#[inline]
fn box_int_if_needed(n: i64) -> i64 {
    if n < 0x10000 {
        return n;
    }
    unsafe {
        let layout = Layout::from_size_align(8, 8).unwrap();
        let ptr = alloc(layout) as *mut i64;
        assert!(!ptr.is_null(), "ocara_runtime: OOM");
        *ptr = n;
        (ptr as i64) | 3
    }
}

/// Convertit n'importe quelle valeur i64 (int, float boxé, bool boxé, string ptr) en String.
fn val_to_string(val: i64) -> String {
    // null (pointeur nul = 0) → affiche "null"
    if val == 0 {
        return "null".to_string();
    }
    if is_float_box(val) {
        unsafe { unbox_float(val).to_string() }
    } else if is_bool_box(val) {
        unsafe { if unbox_bool(val) { "true".to_string() } else { "false".to_string() } }
    } else if is_int_box(val) {
        unsafe { unbox_int(val).to_string() }
    } else if is_ptr(val) {
        unsafe { ptr_to_str(val).to_string() }
    } else {
        val.to_string()
    }
}

/// Formate un i64 en représentation affichable.
fn fmt_val(val: i64) -> String {
    val_to_string(val)
}

// ─────────────────────────────────────────────────────────────────────────────
// Structures heap
// ─────────────────────────────────────────────────────────────────────────────

struct OcaraArray {
    data: Vec<i64>,
}

struct OcaraMap {
    /// Les clés sont stockées comme strings (pointeur i64 → contenu) pour
    /// permettre la comparaison par valeur lors des lookups.
    /// On stocke les paires (clé_str, valeur) pour pouvoir itérer.
    data: Vec<(String, i64)>,
}

fn new_array() -> i64 {
    unsafe {
        let size = std::mem::size_of::<OcaraArray>();
        let layout = Layout::from_size_align(8 + size, 8).unwrap();
        let raw = alloc(layout);
        assert!(!raw.is_null(), "ocara_runtime: OOM (array)");
        *(raw as *mut i64) = TAG_ARRAY;
        let arr_ptr = raw.add(8) as *mut OcaraArray;
        std::ptr::write(arr_ptr, OcaraArray { data: Vec::new() });
        (raw as i64) + 8
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_new() -> i64 {
    new_array()
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_push(ptr: i64, val: i64) {
    if ptr == 0 { return; }
    unsafe { array_ref(ptr).data.push(val); }
}

pub(crate) fn new_map() -> i64 {
    unsafe {
        let size = std::mem::size_of::<OcaraMap>();
        let layout = Layout::from_size_align(8 + size, 8).unwrap();
        let raw = alloc(layout);
        assert!(!raw.is_null(), "ocara_runtime: OOM (map)");
        *(raw as *mut i64) = TAG_MAP;
        let map_ptr = raw.add(8) as *mut OcaraMap;
        std::ptr::write(map_ptr, OcaraMap { data: Vec::new() });
        (raw as i64) + 8
    }
}

unsafe fn array_ref(ptr: i64) -> &'static mut OcaraArray {
    unsafe { &mut *(ptr as *mut OcaraArray) }
}

unsafe fn map_ref(ptr: i64) -> &'static mut OcaraMap {
    unsafe { &mut *(ptr as *mut OcaraMap) }
}

// ─────────────────────────────────────────────────────────────────────────────
// Libération / clonage récursifs — `scoped`/`consumed` (voir docs/EBNF.md et
// le plan "Gestion de propriété des variables"). Portée : string/array/map
// uniquement — un élément TAG_OBJECT/TAG_FUNCTION imbriqué n'est ni libéré
// ni cloné ici (hors périmètre de ce chantier, voir OwnershipClass::Unsupported
// côté sema : ces types ne peuvent pas être `scoped`/`consumed` eux-mêmes,
// mais peuvent apparaître comme élément d'un array/map `scoped`/`consumed`).
// ─────────────────────────────────────────────────────────────────────────────

/// Vrai uniquement pour une string ALLOUÉE SUR LE TAS (`TAG_STRING_OWNED`,
/// posée par `alloc_str`) — pas pour un littéral `.rodata` (`TAG_STRING`).
/// Contrairement à `__is_string` (qui répond vrai pour les deux, une
/// question de TYPE), c'est une question de LIBÉRABILITÉ : seul un
/// `dealloc` sur une adresse réellement `alloc()` est valide.
#[inline]
unsafe fn is_owned_string(val: i64) -> bool {
    unsafe { read_tag(val) == TAG_STRING_OWNED }
}

/// Libère `val` récursivement si c'est un pointeur heap string/array/map.
/// No-op sur tout le reste — **y compris une string littérale** (`.rodata`,
/// tag `TAG_STRING`, pas `TAG_STRING_OWNED`) : c'est ce qui rend cette
/// fonction sûre comme point d'entrée UNIQUE pour la destruction
/// `scoped`/`consumed` (voir `crate::lower::stmt::ownership` côté
/// compilateur) — le type statique AST ne suffit pas à savoir si une valeur
/// `string` donnée est réellement possédée (tas) ou seulement empruntée
/// (littéral figé dans le binaire) ; seul le tag runtime le sait.
#[unsafe(no_mangle)]
pub extern "C" fn __value_free(val: i64) {
    unsafe {
        if is_owned_string(val) { free_str(val); }
        else if __is_array(val) != 0 { __array_free(val); }
        else if __is_map(val)   != 0 { __map_free(val); }
    }
}

/// Clone `val` récursivement si c'est un pointeur heap string/array/map.
/// Retourne `val` tel quel pour tout le reste — ces valeurs n'ont pas de
/// propriétaire distinct à dupliquer (partagées par nature : primitifs,
/// objets, fonctions, 0) — **y compris une string littérale** : immuable et
/// éternelle (vit tout le programme), l'aliaser directement sans copie est
/// toujours sûr, pas besoin d'allouer un clone inutile. Voir `__value_free`.
#[unsafe(no_mangle)]
pub extern "C" fn __value_clone(val: i64) -> i64 {
    unsafe {
        if is_owned_string(val) { alloc_str(ptr_to_str(val)) }
        else if __is_array(val) != 0 { __array_clone(val) }
        else if __is_map(val)   != 0 { __map_clone(val) }
        else { val }
    }
}

/// Libère un array et récursivement chacun de ses éléments tas.
#[unsafe(no_mangle)]
pub extern "C" fn __array_free(ptr: i64) {
    if ptr == 0 { return; }
    unsafe {
        let arr = array_ref(ptr);
        for &el in &arr.data {
            __value_free(el);
        }
        std::ptr::drop_in_place(arr as *mut OcaraArray);
        let size = std::mem::size_of::<OcaraArray>();
        let layout = Layout::from_size_align(8 + size, 8).unwrap();
        dealloc((ptr - 8) as *mut u8, layout);
    }
}

/// Libère une map et récursivement chacune de ses valeurs tas (les clés
/// sont des `String` Rust natifs, libérées avec la map elle-même).
#[unsafe(no_mangle)]
pub extern "C" fn __map_free(ptr: i64) {
    if ptr == 0 { return; }
    unsafe {
        let m = map_ref(ptr);
        for &(_, val) in &m.data {
            __value_free(val);
        }
        std::ptr::drop_in_place(m as *mut OcaraMap);
        let size = std::mem::size_of::<OcaraMap>();
        let layout = Layout::from_size_align(8 + size, 8).unwrap();
        dealloc((ptr - 8) as *mut u8, layout);
    }
}

/// Copie profonde d'un array : nouvel array indépendant, chaque élément tas
/// (string/array/map imbriqué) cloné récursivement — aucune mémoire
/// partagée avec la source.
#[unsafe(no_mangle)]
pub extern "C" fn __array_clone(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe {
        let cloned: Vec<i64> = array_ref(ptr).data.iter()
            .map(|&el| __value_clone(el))
            .collect();
        let new_ptr = new_array();
        array_ref(new_ptr).data = cloned;
        new_ptr
    }
}

/// Copie profonde d'une map : nouvelle map indépendante, clés dupliquées
/// (déjà des `String` Rust natifs) et valeurs tas clonées récursivement.
#[unsafe(no_mangle)]
pub extern "C" fn __map_clone(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe {
        let cloned: Vec<(String, i64)> = map_ref(ptr).data.iter()
            .map(|(k, v)| (k.clone(), __value_clone(*v)))
            .collect();
        let new_ptr = new_map();
        map_ref(new_ptr).data = cloned;
        new_ptr
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Variantes "shallow" (sans inspection des éléments) de free/clone — pour un
// `array<T>`/`map<K,T>` où `T` est un type PRIMITIF CONCRET (int/float/bool),
// jamais `mixed` : voir `crate::lower::stmt::ownership::drop_func_for`/
// `clone_func_for`. `__array_free`/`__array_clone` (ci-dessus) appellent
// `__value_free`/`__value_clone` sur CHAQUE élément, qui inspecte son tag
// runtime via `read_tag` — sûr pour un élément réellement `mixed` (boxé si
// besoin, voir `box_int_if_needed`/`__box_float`/`__box_bool`), mais PAS pour
// un élément primitif brut d'un type concrètement connu : un `float`/`int`
// brut peut avoir n'importe quel bit pattern, y compris un qui ressemble à un
// pointeur heap valide (`val >= PTR_THRESHOLD && bits bas alignés`), auquel
// cas `read_tag` le déréférence — SEGFAULT confirmé par reproduction
// (`var floats:array<float> = [1.5, 2.5, 3.5]` sans aucun `mixed` en jeu,
// jamais échappé : plantait à la libération automatique de fin de bloc). Un
// élément primitif ne possédant jamais de mémoire propre, il n'y a de toute
// façon rien à libérer/cloner récursivement pour lui.
#[unsafe(no_mangle)]
pub extern "C" fn __array_free_shallow(ptr: i64) {
    if ptr == 0 { return; }
    unsafe {
        let arr = array_ref(ptr);
        std::ptr::drop_in_place(arr as *mut OcaraArray);
        let size = std::mem::size_of::<OcaraArray>();
        let layout = Layout::from_size_align(8 + size, 8).unwrap();
        dealloc((ptr - 8) as *mut u8, layout);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __map_free_shallow(ptr: i64) {
    if ptr == 0 { return; }
    unsafe {
        let m = map_ref(ptr);
        std::ptr::drop_in_place(m as *mut OcaraMap);
        let size = std::mem::size_of::<OcaraMap>();
        let layout = Layout::from_size_align(8 + size, 8).unwrap();
        dealloc((ptr - 8) as *mut u8, layout);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_clone_shallow(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe {
        let new_ptr = new_array();
        array_ref(new_ptr).data = array_ref(ptr).data.clone();
        new_ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __map_clone_shallow(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe {
        let new_ptr = new_map();
        map_ref(new_ptr).data = map_ref(ptr).data.clone();
        new_ptr
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
#[unsafe(no_mangle)]
pub extern "C" fn __val_to_str(val: i64) -> i64 {
    if is_float_box(val) || is_bool_box(val) || is_int_box(val) {
        unsafe { alloc_str(&val_to_string(val)) }
    } else if is_ptr(val) {
        val  // déjà une string
    } else {
        unsafe { alloc_str(&val.to_string()) }
    }
}

/// Boxe un float (bits i64) dans une cellule heap ; retourne `ptr | 1`.
#[unsafe(no_mangle)]
pub extern "C" fn __box_float(bits: i64) -> i64 {
    let f = f64::from_bits(bits as u64);
    unsafe {
        let layout = Layout::from_size_align(8, 8).unwrap();
        let ptr = alloc(layout) as *mut f64;
        assert!(!ptr.is_null(), "ocara_runtime: OOM");
        *ptr = f;
        (ptr as i64) | 1
    }
}

/// Boxe un bool (0/1) dans une cellule heap ; retourne `ptr | 2`.
#[unsafe(no_mangle)]
pub extern "C" fn __box_bool(b: i64) -> i64 {
    unsafe {
        let layout = Layout::from_size_align(8, 8).unwrap();
        let ptr = alloc(layout) as *mut i64;
        assert!(!ptr.is_null(), "ocara_runtime: OOM");
        *ptr = b;
        (ptr as i64) | 2
    }
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

#[unsafe(no_mangle)]
pub extern "C" fn __str_from_float(f: f64) -> i64 {
    unsafe { alloc_str(&f.to_string()) }
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
    unsafe { alloc_str(&n.to_string()) }
}

/// Convertit un tableau en string au format `[a, b, c]`.
#[unsafe(no_mangle)]
pub extern "C" fn __array_to_str(ptr: i64) -> i64 {
    if ptr == 0 {
        return unsafe { alloc_str("[]") };
    }
    let parts: Vec<String> = unsafe {
        array_ref(ptr).data.iter().map(|&v| fmt_val(v)).collect()
    };
    let r = format!("[{}]", parts.join(", "));
    unsafe { alloc_str(&r) }
}

/// Retourne le nom du système d'exploitation cible.
#[unsafe(no_mangle)]
pub extern "C" fn __system_os() -> i64 {
    let os = if cfg!(target_os = "linux") {
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
    ocara_print(&f.to_string());
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
    match io::stdin().lock().read_line(&mut line) {
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
    let ptr = new_array();
    unsafe {
        let arr = array_ref(ptr);
        for i in lo..hi {
            arr.data.push(i);
        }
    }
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_len(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { array_ref(ptr).data.len() as i64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_get(ptr: i64, idx: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe {
        let arr = array_ref(ptr);
        let i = idx as usize;
        if i < arr.data.len() { arr.data[i] } else { 0 }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __array_set(ptr: i64, idx: i64, val: i64) {
    if ptr == 0 { return; }
    unsafe {
        let arr = array_ref(ptr);
        let i = idx as usize;
        while arr.data.len() <= i {
            arr.data.push(0);
        }
        arr.data[i] = val;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Maps internes (__map_*)
// ─────────────────────────────────────────────────────────────────────────────

/// Convertit une clé i64 en String pour la comparer par valeur.
/// Si c'est un pointeur vers une string, on lit son contenu.
/// Sinon, on convertit l'entier en string décimale.
unsafe fn key_to_string(key: i64) -> String {
    if is_ptr(key) {
        unsafe { ptr_to_str(key).to_string() }
    } else {
        key.to_string()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __map_new() -> i64 {
    new_map()
}

#[unsafe(no_mangle)]
pub extern "C" fn __map_set(ptr: i64, key: i64, val: i64) {
    if ptr == 0 { return; }
    unsafe {
        let k = key_to_string(key);
        let m = map_ref(ptr);
        // Met à jour si la clé existe déjà
        for entry in &mut m.data {
            if entry.0 == k {
                entry.1 = val;
                return;
            }
        }
        m.data.push((k, val));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __map_get(ptr: i64, key: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe {
        let k = key_to_string(key);
        for entry in &map_ref(ptr).data {
            if entry.0 == k { return entry.1; }
        }
        0
    }
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
    ocara_print(&f.to_string());
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
    ocara_println(&f.to_string());
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

#[unsafe(no_mangle)]
pub extern "C" fn IO_readInt() -> i64 {
    let s = read();
    if s == 0 { return 0; }
    unsafe { ptr_to_str(s).trim().parse::<i64>().unwrap_or(0) }
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readFloat() -> f64 {
    let s = read();
    if s == 0 { return 0.0; }
    unsafe { ptr_to_str(s).trim().parse::<f64>().unwrap_or(0.0) }
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readBool() -> i64 {
    let s = read();
    if s == 0 { return 0; }
    let t = unsafe { ptr_to_str(s).trim().to_lowercase() };
    if t == "true" || t == "1" { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readArray(sep: i64) -> i64 {
    let s = read();
    if s == 0 { return new_array(); }
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep).to_string() } } else { " ".to_string() };
    let src = unsafe { ptr_to_str(s).to_string() };
    let ptr = new_array();
    unsafe {
        let arr = array_ref(ptr);
        for part in src.split(sep_s.as_str()) {
            arr.data.push(alloc_str(part));
        }
    }
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn IO_readMap(sep: i64, kv: i64) -> i64 {
    let s = read();
    if s == 0 { return new_map(); }
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep).to_string() } } else { " ".to_string() };
    let kv_s  = if is_ptr(kv)  { unsafe { ptr_to_str(kv).to_string() }  } else { "=".to_string() };
    let src = unsafe { ptr_to_str(s).to_string() };
    let ptr = new_map();
    for part in src.split(sep_s.as_str()) {
        if let Some(pos) = part.find(kv_s.as_str()) {
            let v = unsafe { alloc_str(&part[pos + kv_s.len()..]) };
            unsafe { __map_set(ptr, alloc_str(&part[..pos]), v); }
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
    unsafe { ptr_to_str(s).chars().count() as i64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_upper(s: i64) -> i64 {
    if !is_ptr(s) { return s; }
    let r = unsafe { ptr_to_str(s) }.to_uppercase();
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_lower(s: i64) -> i64 {
    if !is_ptr(s) { return s; }
    let r = unsafe { ptr_to_str(s) }.to_lowercase();
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_capitalize(s: i64) -> i64 {
    if !is_ptr(s) { return s; }
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
    if !is_ptr(s) { return s; }
    let r = unsafe { ptr_to_str(s) }.trim().to_string();
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_replace(s: i64, from: i64, to: i64) -> i64 {
    if !is_ptr(s) { return s; }
    let src    = unsafe { ptr_to_str(s) };
    let from_s = if is_ptr(from) { unsafe { ptr_to_str(from) } } else { "" };
    let to_s   = if is_ptr(to)   { unsafe { ptr_to_str(to) } }   else { "" };
    let r = src.replacen(from_s, to_s, 1);
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn String_split(s: i64, sep: i64) -> i64 {
    if !is_ptr(s) { return new_array(); }
    let src   = unsafe { ptr_to_str(s).to_string() };
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep).to_string() } } else { " ".to_string() };
    let ptr = new_array();
    unsafe {
        let arr = array_ref(ptr);
        for part in src.split(sep_s.as_str()) {
            arr.data.push(alloc_str(part));
        }
    }
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn String_explode(s: i64, sep: i64) -> i64 {
    String_split(s, sep)
}

#[unsafe(no_mangle)]
pub extern "C" fn String_between(s: i64, start: i64, end: i64) -> i64 {
    if !is_ptr(s) { return s; }
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
    if unsafe { ptr_to_str(s) }.is_empty() { 1 } else { 0 }
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

#[unsafe(no_mangle)]
pub extern "C" fn Math_abs(n: i64) -> i64 { n.abs() }

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
    base.pow(exp as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn Math_clamp(n: i64, lo: i64, hi: i64) -> i64 { n.clamp(lo, hi) }

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
pub extern "C" fn Math_round(n: f64) -> i64 { n.round() as i64 }

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
    if ptr == 0 { return; }
    unsafe { array_ref(ptr).data.push(val); }
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
    unsafe {
        let arr = array_ref(ptr);
        if arr.data.is_empty() {
            exception::throw_array_exception(
                "Cannot pop from empty array",
                ERR_ARRAY_EMPTY,
                "Array"
            );
        }
        arr.data.pop().unwrap()
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
    unsafe {
        let arr = array_ref(ptr);
        if arr.data.is_empty() {
            exception::throw_array_exception(
                "Cannot get first element from empty array",
                ERR_ARRAY_EMPTY,
                "Array"
            );
        }
        *arr.data.first().unwrap()
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
    unsafe {
        let arr = array_ref(ptr);
        if arr.data.is_empty() {
            exception::throw_array_exception(
                "Cannot get last element from empty array",
                ERR_ARRAY_EMPTY,
                "Array"
            );
        }
        *arr.data.last().unwrap()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_contains(ptr: i64, val: i64) -> i64 {
    if ptr == 0 { return 0; }
    if unsafe { array_ref(ptr).data.contains(&val) } { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_indexOf(ptr: i64, val: i64) -> i64 {
    if ptr == 0 { return -1; }
    unsafe {
        array_ref(ptr).data.iter().position(|&x| x == val).map(|i| i as i64).unwrap_or(-1)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_reverse(ptr: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let new_ptr = new_array();
    unsafe {
        let src = array_ref(ptr).data.clone();
        let dst = array_ref(new_ptr);
        dst.data = src.into_iter().rev().collect();
    }
    new_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_slice(ptr: i64, from: i64, to: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let new_ptr = new_array();
    unsafe {
        let src = &array_ref(ptr).data;
        let lo = (from as usize).min(src.len());
        let hi = (to as usize).min(src.len());
        let dst = array_ref(new_ptr);
        dst.data = src[lo..hi].to_vec();
    }
    new_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_join(ptr: i64, sep: i64) -> i64 {
    if ptr == 0 { return unsafe { alloc_str("") }; }
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep).to_string() } } else { "".to_string() };
    let parts: Vec<String> = unsafe {
        array_ref(ptr).data.iter().map(|&v| fmt_val(v)).collect()
    };
    let r = parts.join(&sep_s);
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn Array_sort(ptr: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let new_ptr = new_array();
    unsafe {
        let mut data = array_ref(ptr).data.clone();
        data.sort();
        array_ref(new_ptr).data = data;
    }
    new_ptr
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
    unsafe { map_ref(ptr).data.len() as i64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_has(ptr: i64, key: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe {
        let k = key_to_string(key);
        if map_ref(ptr).data.iter().any(|e| e.0 == k) { 1 } else { 0 }
    }
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
    unsafe {
        let k = key_to_string(key);
        let map = map_ref(ptr);
        match map.data.iter().find(|e| e.0 == k) {
            Some((_, v)) => *v,
            None => {
                exception::throw_map_exception(
                    &format!("Key not found: {}", k),
                    ERR_MAP_KEY_NOT_FOUND,
                    "Map"
                );
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_set(ptr: i64, key: i64, val: i64) {
    __map_set(ptr, key, val);
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_remove(ptr: i64, key: i64) {
    if ptr == 0 { return; }
    unsafe {
        let k = key_to_string(key);
        map_ref(ptr).data.retain(|e| e.0 != k);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_keys(ptr: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let arr_ptr = new_array();
    unsafe {
        // Les clés sont stockées en string — on les alloue
        let keys: Vec<i64> = map_ref(ptr).data.iter()
            .map(|(k, _)| alloc_str(k))
            .collect();
        array_ref(arr_ptr).data = keys;
    }
    arr_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_values(ptr: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let arr_ptr = new_array();
    unsafe {
        let vals: Vec<i64> = map_ref(ptr).data.iter().map(|(_, v)| *v).collect();
        array_ref(arr_ptr).data = vals;
    }
    arr_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_merge(a: i64, b: i64) -> i64 {
    let new_ptr = new_map();
    if a != 0 {
        unsafe {
            for (k, v) in &map_ref(a).data {
                __map_set(new_ptr, alloc_str(k), *v);
            }
        }
    }
    if b != 0 {
        unsafe {
            for (k, v) in &map_ref(b).data {
                __map_set(new_ptr, alloc_str(k), *v);
            }
        }
    }
    new_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Map_isEmpty(ptr: i64) -> i64 {
    if ptr == 0 { return 1; }
    if unsafe { map_ref(ptr).data.is_empty() } { 1 } else { 0 }
}

/// Map::forEach(m, callback) → void
/// `callback` : nameless(key:mixed, value:mixed): void, appelé pour chaque
/// entrée. `callback` est un fat pointer Ocara {func_ptr, env_ptr} (même
/// convention que les handlers HTTPServer::route, voir runtime/src/httpserver.rs).
#[unsafe(no_mangle)]
pub extern "C" fn Map_forEach(ptr: i64, callback: i64) {
    if ptr == 0 || callback == 0 { return; }
    type ForEachFn = extern "C" fn(i64, i64, i64) -> i64;
    let func_ptr = unsafe { *(callback as *const i64) };
    let env_ptr  = unsafe { *(callback as *const i64).add(1) };
    let f: ForEachFn = unsafe { std::mem::transmute(func_ptr as usize) };
    // Copie des entrées avant d'itérer : le callback pourrait modifier la map
    // (Map::set/remove) pendant l'itération, ce qui invaliderait une référence
    // directe vers `data`.
    let entries: Vec<(String, i64)> = unsafe { map_ref(ptr).data.clone() };
    for (k, v) in entries {
        let key_ptr = unsafe { alloc_str(&k) };
        f(env_ptr, key_ptr, v);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Convert
//
// Codes d'erreur ConvertException :
//   101 - INVALID_INT   : Conversion string vers int invalide
//   102 - INVALID_FLOAT : Conversion string vers float invalide
// ─────────────────────────────────────────────────────────────────────────────

const ERR_CONVERT_INVALID_INT: i64 = 101;
const ERR_CONVERT_INVALID_FLOAT: i64 = 102;

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
            unsafe { __map_set(ptr, alloc_str(&part[..pos]), v_str); }
        }
    }
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_intToStr(n: i64) -> i64 {
    unsafe { alloc_str(&n.to_string()) }
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
    unsafe { alloc_str(&f.to_string()) }
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
        let arr = array_ref(ptr);
        for &elem in &arr.data {
            if is_ptr(elem) {
                let s = ptr_to_str(elem).to_string();
                if let Some(pos) = s.find(kv_s.as_str()) {
                    let v = alloc_str(&s[pos + kv_s.len()..]);
                    __map_set(map_ptr, alloc_str(&s[..pos]), v);
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
    let parts: Vec<String> = unsafe {
        map_ref(ptr).data.iter()
            .map(|(k, v)| format!("{}{}{}", k, kv_s, fmt_val(*v)))
            .collect()
    };
    let r = parts.join(&sep_s);
    unsafe { alloc_str(&r) }
}

#[unsafe(no_mangle)]
pub extern "C" fn Convert_mapKeysToArray(ptr: i64) -> i64 {
    Map_keys(ptr)
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
    
    // set_var peut paniquer si le nom ou la valeur contient '=' ou NUL
    // On catch le panic potentiel
    match std::panic::catch_unwind(|| {
        unsafe { std::env::set_var(key, &v); }
    }) {
        Ok(_) => {},
        Err(_) => unsafe {
            exception::throw_system_exception(
                &format!("Failed to set environment variable '{}': invalid name or value", key),
                ERR_SYSTEM_SET_ENV,
                "System"
            );
        }
    }
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
    std::thread::sleep(Duration::from_millis(ms as u64));
}

#[unsafe(no_mangle)]
pub extern "C" fn System_pid() -> i64 {
    std::process::id() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn System_args() -> i64 {
    let ptr = new_array();
    unsafe {
        let arr = array_ref(ptr);
        for arg in std::env::args() {
            arr.data.push(alloc_str(&arg));
        }
    }
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

/// Compile le pattern (i64 ptr → &str) ; lève RegexException si invalide.
unsafe fn compile_regex(pattern: i64) -> Re {
    unsafe {
        let pat = ptr_to_str(pattern);
        match Re::new(pat) {
            Ok(re) => re,
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
            __array_push(arr, ms);
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
            __array_push(arr, ps);
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
fn ut_pass(msg: &str) {
    let s = format!("PASS {}\n", msg);
    write_stdout_raw(s.as_bytes());
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
    if __cmp_eq_strict(expected, actual) != 0 {
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
    if __cmp_eq_strict(expected, actual) == 0 {
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
        ut_pass(&format!("assertGreater: {} > {}", a, b));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertGreater: {} is not > {}", a, b),
                107
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertLess(a: i64, b: i64) {
    if cmp_primitive(a, b, |x, y| x < y, |x, y| x < y) {
        ut_pass(&format!("assertLess: {} < {}", a, b));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertLess: {} is not < {}", a, b),
                108
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertGreaterOrEquals(a: i64, b: i64) {
    if cmp_primitive(a, b, |x, y| x >= y, |x, y| x >= y) {
        ut_pass(&format!("assertGreaterOrEquals: {} >= {}", a, b));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertGreaterOrEquals: {} is not >= {}", a, b),
                109
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertLessOrEquals(a: i64, b: i64) {
    if cmp_primitive(a, b, |x, y| x <= y, |x, y| x <= y) {
        ut_pass(&format!("assertLessOrEquals: {} <= {}", a, b));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertLessOrEquals: {} is not <= {}", a, b),
                110
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertContains(haystack: i64, needle: i64) {
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
    let empty = if value == 0 {
        true
    } else if is_float_box(value) || is_bool_box(value) || is_int_box(value) {
        false
    } else if value >= 0x10000 {
        unsafe { ptr_to_str(value).is_empty() }
    } else {
        value == 0
    };
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
    let empty = if value == 0 {
        true
    } else if is_float_box(value) || is_bool_box(value) || is_int_box(value) {
        false
    } else if value >= 0x10000 {
        unsafe { ptr_to_str(value).is_empty() }
    } else {
        value == 0
    };
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
pub extern "C" fn UnitTest_assertEnum(value: i64) {
    // Les enums sont implémentés comme des objets en Ocara
    let is_obj = __is_object(value) != 0;
    if is_obj {
        ut_pass("assertEnum");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertEnum: value is not an enum", 117);
        }
    }
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
extern "C" fn __assert_raises_body() {
    let func_ptr = ASSERT_RAISES_FUNC_PTR.with(|c| c.get());
    let env_ptr = ASSERT_RAISES_ENV_PTR.with(|c| c.get());
    
    if func_ptr != 0 {
        unsafe {
            let f: OcaraClosureFn = std::mem::transmute(func_ptr as usize);
            f(env_ptr);
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn __assert_raises_handler(error_val: i64, error_type: i64) {
    ASSERT_RAISES_EXCEPTION.with(|e| e.set(error_val));
    ASSERT_RAISES_EXCEPTION_TYPE.with(|t| t.set(error_type));
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

/// Alloue une instance de classe utilisateur avec tag TAG_OBJECT.
/// Le pointeur retourné pointe APRÈS le header, qui fait maintenant 16 octets
/// (au lieu de 8) — un mot supplémentaire est PRÉPENDÉ devant le tag pour y
/// stocker l'identité de classe (`class_id`, attribué une fois par classe à
/// la compilation, voir `IrModule::class_ids`) :
/// ```text
/// avant : [tag:8][données...]
/// après : [class_id:8][tag:8][données...]
/// ```
/// Le tag reste au même offset relatif (`val - 8`), donc invisible de
/// `read_tag`/`__is_object`/tout le reste du runtime — seul `class_id` est
/// nouveau, lu via `*(val - 16)` (voir `Inst::GetField` avec un offset
/// négatif dans le lowering, pas de fonction runtime dédiée). Support du
/// polymorphisme réel (`is ClassName`/`is InterfaceName`, dispatch dynamique
/// d'une méthode appelée via une variable de type parent/interface) — voir
/// docs/roadmap.d/langage-interfaces.md.
///
/// `size == 0` (classe sans aucun champ) reste une allocation VALIDE : même
/// une classe vide a besoin d'un header pour porter son identité — avant ce
/// correctif, `size <= 0` retournait `0` (null), ce qui aurait rendu
/// `self` invalide dans toute méthode d'une classe sans champ dès que le
/// polymorphisme en dépendrait.
#[unsafe(no_mangle)]
pub extern "C" fn __alloc_class_obj(size: i64, class_id: i64) -> i64 {
    if size < 0 { return 0; }
    unsafe {
        let total = (size as usize) + 16;
        let layout = Layout::from_size_align(total, 8).unwrap();
        let raw = alloc_zeroed(layout);
        assert!(!raw.is_null(), "ocara_runtime: OOM in __alloc_class_obj");
        *(raw as *mut i64) = class_id;
        *(raw.add(8) as *mut i64) = TAG_OBJECT;
        (raw as i64) + 16
    }
}

/// Libère une instance de classe utilisateur allouée par `__alloc_class_obj`
/// (tag `TAG_OBJECT`, header de 16 octets — voir sa doc). `n_fields` doit
/// être EXACTEMENT le nombre de champs utilisé à l'allocation — connu
/// statiquement par le compilateur pour chaque classe
/// (`module.class_layouts[Classe].len()`), c'est pourquoi il est passé en
/// argument plutôt que déduit d'un tag/header : rien ne stocke la taille
/// ailleurs. Appelée uniquement depuis un `__free_<Classe>` généré (voir
/// `src/lower/builder.d/class_ownership.rs`), jamais directement — ce n'est
/// PAS un ramasse-miettes général, mêmes précautions que `free_str`.
#[unsafe(no_mangle)]
pub extern "C" fn __object_free(ptr: i64, n_fields: i64) {
    if ptr == 0 { return; }
    unsafe {
        let raw = (ptr - 16) as *mut u8;
        let size = 16 + (n_fields.max(0) as usize) * 8;
        let layout = Layout::from_size_align(size, 8).unwrap();
        dealloc(raw, layout);
    }
}

/// Alloue un fat pointer (Function) avec tag TAG_FUNCTION.
/// 16 octets de données : {func_ptr: i64, env_ptr: i64}.
/// Le pointeur retourné pointe APRÈS le header de 8 octets.
#[unsafe(no_mangle)]
pub extern "C" fn __alloc_fat_ptr() -> i64 {
    unsafe {
        let total = 8 + 16; // header + func_ptr + env_ptr
        let layout = Layout::from_size_align(total, 8).unwrap();
        let raw = alloc_zeroed(layout);
        assert!(!raw.is_null(), "ocara_runtime: OOM in __alloc_fat_ptr");
        *(raw as *mut i64) = TAG_FUNCTION;
        (raw as i64) + 8
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cellule verrouillée pour les variables capturées par une closure/thread
//
// Une variable capturée par une `nameless` est "promue sur le tas" (voir
// `src/lower/expr.d/lower.rs`) : le scope extérieur et la closure partagent
// alors le même pointeur, potentiellement lu/écrit depuis plusieurs threads
// à la fois (Thread::run, workers HTTPServer) — sans ces fonctions, un accès
// concurrent était un comportement non défini (voir
// docs/roadmap.d/memoire-concurrence-threads.md). Chaque cellule embarque
// désormais son propre mutex : `__alloc_locked_cell` réserve les octets d'un
// `pthread_mutex_t` juste AVANT la valeur (même convention "header avant le
// pointeur retourné" que les tags `TAG_*` du reste du runtime), et
// `__locked_cell_get`/`__locked_cell_set` remplacent tout accès direct
// (Load/Store) à une variable capturée dans le lowering.
//
// Mutex séparé de celui de `ocara.Mutex` (mutex.rs) : plus petit, à usage
// interne uniquement (jamais exposé au langage), pas de gestion d'exception.

#[cfg(target_os = "linux")]
type CapturedCellMutex = [u8; 40];
#[cfg(target_os = "macos")]
type CapturedCellMutex = [u8; 64];
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
type CapturedCellMutex = [u8; 64];

unsafe extern "C" {
    fn pthread_mutex_init(mutex: *mut CapturedCellMutex, attr: *const u8) -> i32;
    fn pthread_mutex_lock(mutex: *mut CapturedCellMutex) -> i32;
    fn pthread_mutex_unlock(mutex: *mut CapturedCellMutex) -> i32;
}

const CAPTURED_CELL_MUTEX_SIZE: usize = std::mem::size_of::<CapturedCellMutex>();

/// Alloue une cellule de capture verrouillée : `[mutex][valeur: i64]`.
/// Retourne un pointeur vers la valeur (le mutex vit juste avant, à
/// `retour - CAPTURED_CELL_MUTEX_SIZE`) — jamais libérée (même limite que
/// `__alloc_obj` pour une closure : voir docs/roadmap.d/memoire-strategie-var.md).
#[unsafe(no_mangle)]
pub extern "C" fn __alloc_locked_cell() -> i64 {
    unsafe {
        let total = CAPTURED_CELL_MUTEX_SIZE + 8;
        let layout = Layout::from_size_align(total, 8).unwrap();
        let raw = alloc_zeroed(layout);
        assert!(!raw.is_null(), "ocara_runtime: OOM in __alloc_locked_cell");
        pthread_mutex_init(raw as *mut CapturedCellMutex, std::ptr::null());
        (raw as i64) + CAPTURED_CELL_MUTEX_SIZE as i64
    }
}

/// Lit la valeur d'une cellule de capture sous verrou.
#[unsafe(no_mangle)]
pub extern "C" fn __locked_cell_get(cell_ptr: i64) -> i64 {
    unsafe {
        let mutex_ptr = (cell_ptr - CAPTURED_CELL_MUTEX_SIZE as i64) as *mut CapturedCellMutex;
        pthread_mutex_lock(mutex_ptr);
        let val = *(cell_ptr as *const i64);
        pthread_mutex_unlock(mutex_ptr);
        val
    }
}

/// Écrit la valeur d'une cellule de capture sous verrou.
#[unsafe(no_mangle)]
pub extern "C" fn __locked_cell_set(cell_ptr: i64, val: i64) {
    unsafe {
        let mutex_ptr = (cell_ptr - CAPTURED_CELL_MUTEX_SIZE as i64) as *mut CapturedCellMutex;
        pthread_mutex_lock(mutex_ptr);
        *(cell_ptr as *mut i64) = val;
        pthread_mutex_unlock(mutex_ptr);
    }
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
    #[allow(improper_ctypes)]
    fn setjmp(env: *mut JmpBuf) -> i32;
    #[allow(improper_ctypes)]
    fn longjmp(env: *mut JmpBuf, val: i32) -> !;
}

// État de la pile try (par thread, profondeur max = 64)
struct TryFrame {
    env:        JmpBuf,
    error_val:  i64,
    error_type: i64,
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

// État pour propager les returns des handlers d'exception
struct HandlerReturnState {
    has_returned: Cell<bool>,
    return_value: Cell<i64>,
}

thread_local! {
    static HANDLER_RETURN: HandlerReturnState = HandlerReturnState {
        has_returned: Cell::new(false),
        return_value: Cell::new(0),
    };
}

/// Appelée par le handler avant de faire return pour signaler qu'il veut propager
#[unsafe(no_mangle)]
pub extern "C" fn __ocara_handler_set_return(value: i64) {
    HANDLER_RETURN.with(|state| {
        state.has_returned.set(true);
        state.return_value.set(value);
    });
}

/// Appelée par __ocara_try_exec pour vérifier si le handler a fait return
fn handler_has_returned() -> (bool, i64) {
    HANDLER_RETURN.with(|state| {
        let has_ret = state.has_returned.get();
        let val = state.return_value.get();
        // Reset pour le prochain try/on
        state.has_returned.set(false);
        state.return_value.set(0);
        (has_ret, val)
    })
}

/// Exécute un bloc try/on. Retourne 0 si le bloc se termine normalement,
/// ou (1 + return_value) si le handler a fait un return explicite.
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
        }

        // Pousser la nouvelle profondeur
        stack.depth.set(depth + 1);

        // SETJMP — sauvegarde la frame de __ocara_try_exec
        let env_ptr: *mut JmpBuf = unsafe { &mut (*frame_ptr).env };
        let ret = unsafe { setjmp(env_ptr) };

        if ret == 0 {
            // Exécution normale du corps try
            unsafe {
                let body: unsafe extern "C" fn() =
                    std::mem::transmute(body_fn as usize);
                body();
            }
            // Sortie normale : dépiler
            stack.depth.set(depth);
            0  // Pas de return du handler
        } else {
            // longjmp déclenché : récupérer error_val et error_type
            let (ev, et) = unsafe {
                ((*frame_ptr).error_val, (*frame_ptr).error_type)
            };
            // Dépiler avant d'appeler le handler
            stack.depth.set(depth);
            // Appeler le gestionnaire
            unsafe {
                let handler: unsafe extern "C" fn(i64, i64) =
                    std::mem::transmute(handler_fn as usize);
                handler(ev, et);
            }
            
            // Vérifier si le handler a fait un return explicite
            let (has_returned, return_value) = handler_has_returned();
            if has_returned {
                // Encoder : 1 + valeur pour distinguer de 0 (pas de return)
                return_value.wrapping_add(1)
            } else {
                0  // Pas de return
            }
        }
    })
}

/// Version dynamique avec pointeur vers tableau de captures.
/// Le body reçoit un *const i64 pointant vers le début du tableau.
/// Retourne 0 si le bloc se termine normalement, ou (1 + return_value) si return.
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
        }

        stack.depth.set(depth + 1);

        let env_ptr: *mut JmpBuf = unsafe { &mut (*frame_ptr).env };
        let ret = unsafe { setjmp(env_ptr) };

        if ret == 0 {
            // Le body reçoit le pointeur vers les captures
            unsafe {
                let body: unsafe extern "C" fn(*const i64) =
                    std::mem::transmute(body_fn as usize);
                body(captures_ptr);
            }
            stack.depth.set(depth);
            0  // Pas de return
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
            unsafe {
                let handler: unsafe extern "C" fn(i64, i64, *const i64) =
                    std::mem::transmute(handler_fn as usize);
                handler(ev, et, captures_ptr);
            }

            // Vérifier si le handler a fait un return explicite
            let (has_returned, return_value) = handler_has_returned();
            if has_returned {
                // Encoder : 1 + valeur
                return_value.wrapping_add(1)
            } else {
                0  // Pas de return
            }
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
        std::process::exit(1);
    }
}

/// `stored` porte la chaîne d'ancêtres de la classe réellement levée (elle-
/// même incluse), du plus spécifique au plus général, jointe par `|` (ex.
/// `"FileNotFound|FileException|Exception"` — voir IrModule::ancestor_chain,
/// lower_raise). Un filtre sur une classe PARENTE doit attraper une sous-
/// classe : on cherche `filter` comme un des maillons de la chaîne, pas une
/// égalité stricte avec la valeur entière de `stored` — voir
/// docs/roadmap.d/langage-exceptions.md. Reste compatible avec un `stored`
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
// Async tasks
// ─────────────────────────────────────────────────────────────────────────────

struct OcaraTask {
    handle: Option<std::thread::JoinHandle<i64>>,
}

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
    let ptr = (tagged & !3) as *const i64;
    unsafe { *ptr }
}

/// Déboxe un int précédemment boxé par `__box_int_for_mixed`.
/// Entrée : `ptr | 3` (tagged heap pointer).
/// Sortie : la valeur i64 originale.
#[unsafe(no_mangle)]
pub extern "C" fn __unbox_int(tagged: i64) -> i64 {
    let ptr = (tagged & !3) as *const i64;
    unsafe { *ptr }
}

#[unsafe(no_mangle)]
pub extern "C" fn __task_spawn(func: i64, env: i64) -> i64 {
    let handle = std::thread::spawn(move || unsafe {
        let f: extern "C" fn(i64) -> i64 = std::mem::transmute(func as usize);
        f(env)
    });
    let task = Box::new(OcaraTask { handle: Some(handle) });
    Box::into_raw(task) as i64
}

// `resolve expr` : attend le thread et libère le wrapper OcaraTask (jusqu'ici
// jamais libéré, même sur ce chemin nominal — le pointeur n'était que
// déréférencé via `&mut *`, jamais repris via `Box::from_raw`). Pas de piège
// longjmp ici : `handle.join()` ne lève pas d'exception Ocara (échec avalé
// par `unwrap_or(0)`), donc un drop de fin de scope classique est sûr.
// Limite assumée (hors périmètre) : une tâche jamais `resolve`e fuit toujours
// — rien n'impose l'appel de `resolve` côté Ocara, pas d'équivalent `detach`
// pour les tâches async.
#[unsafe(no_mangle)]
pub extern "C" fn __task_resolve(task_ptr: i64) -> i64 {
    if task_ptr == 0 {
        return 0;
    }
    let mut task = unsafe { Box::from_raw(task_ptr as *mut OcaraTask) };
    if let Some(handle) = task.handle.take() {
        handle.join().unwrap_or(0)
    } else {
        0
    }
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
fn get_value_type(val: i64) -> i32 {
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

    // Types primitifs (int/float/bool, bruts ou boxés) : voir cmp_primitive
    if lhs_type == 1 {
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
/// aurait sinon été silencieusement tronqué en entier (voir
/// docs/roadmap.d/langage-mixed-arithmetic.md). Retourne, comme `__dyn_add`,
/// une valeur "mixed" auto-décrite (entier brut ou float boxé).
macro_rules! dyn_arith_op {
    ($name:ident, $op:tt) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(a: i64, b: i64) -> i64 {
            if is_float_box(a) || is_float_box(b) {
                __box_float((unbox_numeric_f64(a) $op unbox_numeric_f64(b)).to_bits() as i64)
            } else {
                // Voir le commentaire équivalent dans `__dyn_add` : le résultat
                // est déjà une valeur "mixed" retournée telle quelle à
                // l'appelant, donc à reboxer ICI si nécessaire.
                box_int_if_needed(unbox_numeric_i64(a) $op unbox_numeric_i64(b))
            }
        }
    };
}
dyn_arith_op!(__dyn_sub, -);
dyn_arith_op!(__dyn_mul, *);
dyn_arith_op!(__dyn_div, /);

// ────────────────────────────────────────────────────────────────────────────
// ocara.JSON — Sérialisation et désérialisation JSON
// ────────────────────────────────────────────────────────────────────────────

use serde_json::{Value as JsonValue, Map as JsonMap};

/// JSON::encode(data) → string
/// Encode un array ou map en JSON
#[unsafe(no_mangle)]
pub extern "C" fn JSON_encode(data: i64) -> i64 {
    if data == 0 {
        return unsafe { alloc_str("null") };
    }
    
    let typ = get_value_type(data);
    
    match typ {
        5 => {  // TAG_ARRAY (valeur = 5 selon get_value_type)
            encode_array_to_json(data)
        }
        6 => {  // TAG_MAP (valeur = 6 selon get_value_type)
            encode_map_to_json(data)
        }
        _ => {
            // Type non supporté pour encode, retourner une chaîne vide
            unsafe { alloc_str("") }
        }
    }
}

/// Encode récursivement un array Ocara en JSON
fn encode_array_to_json(arr: i64) -> i64 {
    let mut json_arr = Vec::new();
    let len = __array_len(arr);
    
    for i in 0..len {
        let elem = __array_get(arr, i);
        let json_val = value_to_json(elem);
        json_arr.push(json_val);
    }
    
    let json_str = serde_json::to_string(&json_arr).unwrap_or_else(|_| "[]".to_string());
    unsafe { alloc_str(&json_str) }
}

/// Encode récursivement une map Ocara en JSON
fn encode_map_to_json(map: i64) -> i64 {
    let mut json_obj = JsonMap::new();
    
    // Parcourir les clés de la map
    unsafe {
        let map_ptr = map as *mut OcaraMap;
        for (key_str, value) in (*map_ptr).data.iter() {
            let json_val = value_to_json(*value);
            json_obj.insert(key_str.clone(), json_val);
        }
    }
    
    let json_str = serde_json::to_string(&json_obj).unwrap_or_else(|_| "{}".to_string());
    unsafe { alloc_str(&json_str) }
}

/// Convertit une valeur Ocara en JsonValue
fn value_to_json(val: i64) -> JsonValue {
    if val == 0 {
        return JsonValue::Null;
    }

    // `float`/`bool` boxés (voir `__box_float`/`__box_bool`) : à vérifier
    // AVANT `get_value_type`, qui les classe tous les deux (avec un `int`
    // brut) dans le même panier "primitif" (1) sans les distinguer — sans ce
    // déballage, un float/bool construit par un littéral `array<mixed>`/
    // `map<string,mixed>` (voir lower_array_literal/lower_map_literal)
    // ressortait comme un entier correspondant à l'adresse du pointeur boxé
    // (confirmé par reproduction — voir
    // docs/roadmap.d/langage-mixed-literal-stringification.md).
    if is_float_box(val) {
        let f = unsafe { unbox_float(val) };
        return serde_json::Number::from_f64(f).map(JsonValue::Number).unwrap_or(JsonValue::Null);
    }
    if is_bool_box(val) {
        return JsonValue::Bool(unsafe { unbox_bool(val) });
    }
    // `int` boxé (voir `box_int_if_needed`) : même raison de le vérifier AVANT
    // `get_value_type`, qui le classerait aussi "primitif" (1) sans le
    // distinguer d'un vrai entier brut de même magnitude.
    if is_int_box(val) {
        return JsonValue::Number(serde_json::Number::from(unsafe { unbox_int(val) }));
    }

    let typ = get_value_type(val);

    match typ {
        1 => {  // Primitif : entier brut (float/bool boxés déjà traités ci-dessus).
            // `val == 1`/`== 0` reste une heuristique imprécise pour un
            // bool JAMAIS boxé (limitation pré-existante, non résolue ici —
            // voir __is_bool) ; conservée telle quelle pour ne rien changer
            // au comportement déjà en place pour ce cas résiduel.
            if val == 1 {  // true
                JsonValue::Bool(true)
            } else if val == 0 {  // false (mais déjà traité par le test au début)
                JsonValue::Bool(false)
            } else {
                JsonValue::Number(serde_json::Number::from(val))
            }
        }
        4 => {  // String
            JsonValue::String(unsafe { ptr_to_str(val) }.to_string())
        }
        5 => {  // Array
            let mut json_arr = Vec::new();
            let len = __array_len(val);
            for i in 0..len {
                let elem = __array_get(val, i);
                json_arr.push(value_to_json(elem));
            }
            JsonValue::Array(json_arr)
        }
        6 => {  // Map
            let mut json_obj = JsonMap::new();
            unsafe {
                let map_ptr = val as *mut OcaraMap;
                for (key_str, value) in (*map_ptr).data.iter() {
                    json_obj.insert(key_str.clone(), value_to_json(*value));
                }
            }
            JsonValue::Object(json_obj)
        }
        _ => JsonValue::Null
    }
}

/// JSON::decode(json) → mixed (array ou map)
/// Décode une string JSON en structure Ocara
#[unsafe(no_mangle)]
pub extern "C" fn JSON_decode(json: i64) -> i64 {
    if json == 0 {
        return 0;
    }
    
    let json_str = unsafe { ptr_to_str(json) };
    
    match serde_json::from_str::<JsonValue>(json_str) {
        Ok(value) => json_to_value(&value),
        Err(_) => 0  // Retourner null en cas d'erreur
    }
}

/// Convertit un JsonValue en valeur Ocara
fn json_to_value(json: &JsonValue) -> i64 {
    match json {
        JsonValue::Null => 0,
        JsonValue::Bool(b) => if *b { 1 } else { 0 },
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                // La valeur décodée est toujours logée dans un `mixed`
                // (élément d'array/map, voir docs `JSON::decode`) — boxer si
                // besoin, comme n'importe quel autre `int` en transit vers un
                // `mixed` (voir `box_int_if_needed`).
                box_int_if_needed(i)
            } else {
                0
            }
        }
        JsonValue::String(s) => unsafe { alloc_str(s) },
        JsonValue::Array(arr) => {
            let ocara_arr = __array_new();
            for elem in arr {
                let ocara_val = json_to_value(elem);
                __array_push(ocara_arr, ocara_val);
            }
            ocara_arr
        }
        JsonValue::Object(obj) => {
            let ocara_map = __map_new();
            for (key, value) in obj {
                let key_str = unsafe { alloc_str(key) };
                let ocara_val = json_to_value(value);
                __map_set(ocara_map, key_str, ocara_val);
            }
            ocara_map
        }
    }
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
        Err(_) => json  // Retourner la string originale en cas d'erreur
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
        Err(_) => json  // Retourner la string originale en cas d'erreur
    }
}
