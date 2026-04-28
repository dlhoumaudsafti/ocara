// ─────────────────────────────────────────────────────────────────────────────
// ocara_runtime — bibliothèque runtime d'Ocara v1.0
//
// Toutes les fonctions sont exportées avec la convention C (`extern "C"`,
// `#[no_mangle]`) pour être liées aux binaires produits par le compilateur.
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
//   comme des pointeurs. Pour ces cas, utiliser Convert::int_to_str() d'abord.
// ─────────────────────────────────────────────────────────────────────────────

#![allow(clippy::missing_safety_doc)]

use std::alloc::{alloc, alloc_zeroed, Layout};
use crate::typecheck::{TAG_STRING, TAG_ARRAY, TAG_MAP, TAG_OBJECT, TAG_FUNCTION, __is_function, __is_object, __is_map, __is_array};
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
pub mod http;
pub mod thread;
pub mod mutex;
pub mod httpserver;
pub mod datetime;
pub mod date;
pub mod time;
pub mod typecheck;
pub mod htmlcomponent;

// Helpers mémoire internes
// ─────────────────────────────────────────────────────────────────────────────

/// Alloue une chaîne null-terminated sur le heap et retourne son adresse.
/// Alignement 8 pour garantir que les 3 bits bas sont 0 (invariant boxing).
pub(crate) unsafe fn alloc_str(s: &str) -> i64 {
    let bytes = s.as_bytes();
    // 8 octets header (tag) + données + null-terminator
    let total = 8 + bytes.len() + 1;
    let layout = Layout::from_size_align(total, 8).unwrap();
    let raw = alloc(layout);
    assert!(!raw.is_null(), "ocara_runtime: OOM");
    // Écrire le tag dans le header
    *(raw as *mut i64) = TAG_STRING;
    // Copier les données de la chaîne après le header
    let data = raw.add(8);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
    *data.add(bytes.len()) = 0u8;
    // Retourner le pointeur APRÈS le header (= pointeur vers les données)
    (raw as i64) + 8
}

/// Lit un pointeur i64 comme &str (null-terminated UTF-8).
pub(crate) unsafe fn ptr_to_str<'a>(val: i64) -> &'a str {
    if val == 0 {
        return "";
    }
    let cstr = CStr::from_ptr(val as *const i8);
    cstr.to_str().unwrap_or("")
}

// ─── Tagged-pointer boxing pour les valeurs `any` ───────────────────────────
// Les pointeurs heap sont alignés sur 8 octets → les 3 bits bas sont 0.
// On encode le type dans les 2 bits bas :
//   bits 1:0 = 00  → string ou objet normal (is_ptr)
//   bits 1:0 = 01  → float boxé  (__box_float)
//   bits 1:0 = 10  → bool boxé   (__box_bool)

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

#[inline]
unsafe fn unbox_float(val: i64) -> f64 {
    *((val & !3) as *const f64)
}

#[inline]
unsafe fn unbox_bool(val: i64) -> bool {
    *((val & !3) as *const i64) != 0
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

#[no_mangle]
pub extern "C" fn __array_new() -> i64 {
    new_array()
}

#[no_mangle]
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
    &mut *(ptr as *mut OcaraArray)
}

unsafe fn map_ref(ptr: i64) -> &'static mut OcaraMap {
    &mut *(ptr as *mut OcaraMap)
}

// ─────────────────────────────────────────────────────────────────────────────
// I/O de base — write / read
// ─────────────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn write(val: i64) {
    ocara_print(&fmt_val(val));
}

#[no_mangle]
pub extern "C" fn __str_concat(a: i64, b: i64) -> i64 {
    let sa = val_to_string(a);
    let sb = val_to_string(b);
    unsafe { alloc_str(&(sa + &sb)) }
}

/// Convertit n'importe quelle valeur I64 en string (pour les templates).
#[no_mangle]
pub extern "C" fn __val_to_str(val: i64) -> i64 {
    if is_float_box(val) || is_bool_box(val) {
        unsafe { alloc_str(&val_to_string(val)) }
    } else if is_ptr(val) {
        val  // déjà une string
    } else {
        unsafe { alloc_str(&val.to_string()) }
    }
}

/// Boxe un float (bits i64) dans une cellule heap ; retourne `ptr | 1`.
#[no_mangle]
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
#[no_mangle]
pub extern "C" fn __box_bool(b: i64) -> i64 {
    unsafe {
        let layout = Layout::from_size_align(8, 8).unwrap();
        let ptr = alloc(layout) as *mut i64;
        assert!(!ptr.is_null(), "ocara_runtime: OOM");
        *ptr = b;
        (ptr as i64) | 2
    }
}

#[no_mangle]
pub extern "C" fn __str_from_float(f: f64) -> i64 {
    unsafe { alloc_str(&f.to_string()) }
}

#[no_mangle]
pub extern "C" fn __str_from_bool(b: i64) -> i64 {
    unsafe { alloc_str(if b != 0 { "true" } else { "false" }) }
}

/// Convertit un entier i64 en string — sans heuristique pointeur.
/// À utiliser dans les templates `${expr}` quand le type est I64.
#[no_mangle]
pub extern "C" fn __str_from_int(n: i64) -> i64 {
    unsafe { alloc_str(&n.to_string()) }
}

/// Convertit un tableau en string au format `[a, b, c]`.
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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

#[no_mangle]
pub extern "C" fn write_int(n: i64) {
    ocara_print(&n.to_string());
}

#[no_mangle]
pub extern "C" fn write_float(f: f64) {
    ocara_print(&f.to_string());
}

#[no_mangle]
pub extern "C" fn write_bool(b: i64) {
    ocara_print(if b != 0 { "true" } else { "false" });
}

/// Fonction interne (pas exportée en C) — lit une ligne sur stdin.
/// NOTE : pas de #[no_mangle] pour éviter de shadower le `read(fd,buf,n)` POSIX
///        dont Rust's io::stdin() a besoin en interne.
fn read() -> i64 {
    let mut line = String::new();
    let _ = io::stdin().lock().read_line(&mut line);
    if line.ends_with('\n') { line.pop(); }
    if line.ends_with('\r') { line.pop(); }
    unsafe { alloc_str(&line) }
}

/// Alias exporté pour les programmes Ocara qui appellent `read()` directement
#[no_mangle]
pub extern "C" fn ocara_read() -> i64 {
    read()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tableaux internes (__array_*, __range)
// ─────────────────────────────────────────────────────────────────────────────

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn __array_len(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { array_ref(ptr).data.len() as i64 }
}

#[no_mangle]
pub extern "C" fn __array_get(ptr: i64, idx: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe {
        let arr = array_ref(ptr);
        let i = idx as usize;
        if i < arr.data.len() { arr.data[i] } else { 0 }
    }
}

#[no_mangle]
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
        ptr_to_str(key).to_string()
    } else {
        key.to_string()
    }
}

#[no_mangle]
pub extern "C" fn __map_new() -> i64 {
    new_map()
}

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn __map_foreach(_ptr: i64, _cb: i64, _ctx: i64) {
    // TODO : implémentation complète nécessite le support des pointeurs de fonctions
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.IO
// ─────────────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn IO_write(val: i64) {
    ocara_print(&fmt_val(val));
}

#[no_mangle]
pub extern "C" fn IO_write_int(n: i64) {
    ocara_print(&n.to_string());
}

#[no_mangle]
pub extern "C" fn IO_write_float(f: f64) {
    ocara_print(&f.to_string());
}

#[no_mangle]
pub extern "C" fn IO_write_bool(b: i64) {
    ocara_print(if b != 0 { "true" } else { "false" });
}

#[no_mangle]
pub extern "C" fn IO_writeln(val: i64) {
    ocara_println(&fmt_val(val));
}

#[no_mangle]
pub extern "C" fn IO_writeln_int(n: i64) {
    ocara_println(&n.to_string());
}

#[no_mangle]
pub extern "C" fn IO_writeln_float(f: f64) {
    ocara_println(&f.to_string());
}

#[no_mangle]
pub extern "C" fn IO_writeln_bool(b: i64) {
    ocara_println(if b != 0 { "true" } else { "false" });
}

#[no_mangle]
pub extern "C" fn IO_read() -> i64 {
    read()
}

#[no_mangle]
pub extern "C" fn IO_readln() -> i64 {
    read()
}

#[no_mangle]
pub extern "C" fn IO_read_int() -> i64 {
    let s = read();
    if s == 0 { return 0; }
    unsafe { ptr_to_str(s).trim().parse::<i64>().unwrap_or(0) }
}

#[no_mangle]
pub extern "C" fn IO_read_float() -> f64 {
    let s = read();
    if s == 0 { return 0.0; }
    unsafe { ptr_to_str(s).trim().parse::<f64>().unwrap_or(0.0) }
}

#[no_mangle]
pub extern "C" fn IO_read_bool() -> i64 {
    let s = read();
    if s == 0 { return 0; }
    let t = unsafe { ptr_to_str(s).trim().to_lowercase() };
    if t == "true" || t == "1" { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn IO_read_array(sep: i64) -> i64 {
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

#[no_mangle]
pub extern "C" fn IO_read_map(sep: i64, kv: i64) -> i64 {
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

#[no_mangle]
pub extern "C" fn String_len(s: i64) -> i64 {
    if !is_ptr(s) { return 0; }
    unsafe { ptr_to_str(s).chars().count() as i64 }
}

#[no_mangle]
pub extern "C" fn String_upper(s: i64) -> i64 {
    if !is_ptr(s) { return s; }
    let r = unsafe { ptr_to_str(s) }.to_uppercase();
    unsafe { alloc_str(&r) }
}

#[no_mangle]
pub extern "C" fn String_lower(s: i64) -> i64 {
    if !is_ptr(s) { return s; }
    let r = unsafe { ptr_to_str(s) }.to_lowercase();
    unsafe { alloc_str(&r) }
}

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn String_trim(s: i64) -> i64 {
    if !is_ptr(s) { return s; }
    let r = unsafe { ptr_to_str(s) }.trim().to_string();
    unsafe { alloc_str(&r) }
}

#[no_mangle]
pub extern "C" fn String_replace(s: i64, from: i64, to: i64) -> i64 {
    if !is_ptr(s) { return s; }
    let src    = unsafe { ptr_to_str(s) };
    let from_s = if is_ptr(from) { unsafe { ptr_to_str(from) } } else { "" };
    let to_s   = if is_ptr(to)   { unsafe { ptr_to_str(to) } }   else { "" };
    let r = src.replacen(from_s, to_s, 1);
    unsafe { alloc_str(&r) }
}

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn String_explode(s: i64, sep: i64) -> i64 {
    String_split(s, sep)
}

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn String_empty(s: i64) -> i64 {
    if !is_ptr(s) { return 1; }
    if unsafe { ptr_to_str(s) }.is_empty() { 1 } else { 0 }
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Math
// ─────────────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn Math_abs(n: i64) -> i64 { n.abs() }

#[no_mangle]
pub extern "C" fn Math_min(a: i64, b: i64) -> i64 { a.min(b) }

#[no_mangle]
pub extern "C" fn Math_max(a: i64, b: i64) -> i64 { a.max(b) }

#[no_mangle]
pub extern "C" fn Math_pow(base: i64, exp: i64) -> i64 {
    if exp < 0 { return 0; }
    base.pow(exp as u32)
}

#[no_mangle]
pub extern "C" fn Math_clamp(n: i64, lo: i64, hi: i64) -> i64 { n.clamp(lo, hi) }

#[no_mangle]
pub extern "C" fn Math_sqrt(n: f64) -> f64 { n.sqrt() }

#[no_mangle]
pub extern "C" fn Math_floor(n: f64) -> i64 { n.floor() as i64 }

#[no_mangle]
pub extern "C" fn Math_ceil(n: f64) -> i64 { n.ceil() as i64 }

#[no_mangle]
pub extern "C" fn Math_round(n: f64) -> i64 { n.round() as i64 }

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Array
// ─────────────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn Array_len(ptr: i64) -> i64 {
    __array_len(ptr)
}

#[no_mangle]
pub extern "C" fn Array_push(ptr: i64, val: i64) {
    if ptr == 0 { return; }
    unsafe { array_ref(ptr).data.push(val); }
}

#[no_mangle]
pub extern "C" fn Array_pop(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { array_ref(ptr).data.pop().unwrap_or(0) }
}

#[no_mangle]
pub extern "C" fn Array_first(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { *array_ref(ptr).data.first().unwrap_or(&0) }
}

#[no_mangle]
pub extern "C" fn Array_last(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { *array_ref(ptr).data.last().unwrap_or(&0) }
}

#[no_mangle]
pub extern "C" fn Array_contains(ptr: i64, val: i64) -> i64 {
    if ptr == 0 { return 0; }
    if unsafe { array_ref(ptr).data.contains(&val) } { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Array_index_of(ptr: i64, val: i64) -> i64 {
    if ptr == 0 { return -1; }
    unsafe {
        array_ref(ptr).data.iter().position(|&x| x == val).map(|i| i as i64).unwrap_or(-1)
    }
}

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Array_join(ptr: i64, sep: i64) -> i64 {
    if ptr == 0 { return unsafe { alloc_str("") }; }
    let sep_s = if is_ptr(sep) { unsafe { ptr_to_str(sep).to_string() } } else { "".to_string() };
    let parts: Vec<String> = unsafe {
        array_ref(ptr).data.iter().map(|&v| fmt_val(v)).collect()
    };
    let r = parts.join(&sep_s);
    unsafe { alloc_str(&r) }
}

#[no_mangle]
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

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Map
// ─────────────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn Map_size(ptr: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe { map_ref(ptr).data.len() as i64 }
}

#[no_mangle]
pub extern "C" fn Map_has(ptr: i64, key: i64) -> i64 {
    if ptr == 0 { return 0; }
    unsafe {
        let k = key_to_string(key);
        if map_ref(ptr).data.iter().any(|e| e.0 == k) { 1 } else { 0 }
    }
}

#[no_mangle]
pub extern "C" fn Map_get(ptr: i64, key: i64) -> i64 {
    __map_get(ptr, key)
}

#[no_mangle]
pub extern "C" fn Map_set(ptr: i64, key: i64, val: i64) {
    __map_set(ptr, key, val);
}

#[no_mangle]
pub extern "C" fn Map_remove(ptr: i64, key: i64) {
    if ptr == 0 { return; }
    unsafe {
        let k = key_to_string(key);
        map_ref(ptr).data.retain(|e| e.0 != k);
    }
}

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Map_values(ptr: i64) -> i64 {
    if ptr == 0 { return new_array(); }
    let arr_ptr = new_array();
    unsafe {
        let vals: Vec<i64> = map_ref(ptr).data.iter().map(|(_, v)| *v).collect();
        array_ref(arr_ptr).data = vals;
    }
    arr_ptr
}

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Map_is_empty(ptr: i64) -> i64 {
    if ptr == 0 { return 1; }
    if unsafe { map_ref(ptr).data.is_empty() } { 1 } else { 0 }
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Convert
// ─────────────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn Convert_str_to_int(s: i64) -> i64 {
    if !is_ptr(s) { return 0; }
    unsafe { ptr_to_str(s).trim().parse::<i64>().unwrap_or(0) }
}

#[no_mangle]
pub extern "C" fn Convert_str_to_float(s: i64) -> f64 {
    if !is_ptr(s) { return 0.0; }
    unsafe { ptr_to_str(s).trim().parse::<f64>().unwrap_or(0.0) }
}

#[no_mangle]
pub extern "C" fn Convert_str_to_bool(s: i64) -> i64 {
    if !is_ptr(s) { return 0; }
    let t = unsafe { ptr_to_str(s).trim().to_lowercase() };
    if t == "true" || t == "1" { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Convert_str_to_array(s: i64, sep: i64) -> i64 {
    String_split(s, sep)
}

#[no_mangle]
pub extern "C" fn Convert_str_to_map(s: i64, sep: i64, kv: i64) -> i64 {
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

#[no_mangle]
pub extern "C" fn Convert_int_to_str(n: i64) -> i64 {
    unsafe { alloc_str(&n.to_string()) }
}

#[no_mangle]
pub extern "C" fn Convert_int_to_float(n: i64) -> f64 {
    n as f64
}

#[no_mangle]
pub extern "C" fn Convert_int_to_bool(n: i64) -> i64 {
    if n != 0 { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Convert_float_to_str(f: f64) -> i64 {
    unsafe { alloc_str(&f.to_string()) }
}

#[no_mangle]
pub extern "C" fn Convert_float_to_int(f: f64) -> i64 {
    f as i64
}

#[no_mangle]
pub extern "C" fn Convert_float_to_bool(f: f64) -> i64 {
    if f != 0.0 { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Convert_bool_to_str(b: i64) -> i64 {
    unsafe { alloc_str(if b != 0 { "true" } else { "false" }) }
}

#[no_mangle]
pub extern "C" fn Convert_bool_to_int(b: i64) -> i64 {
    if b != 0 { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Convert_bool_to_float(b: i64) -> f64 {
    if b != 0 { 1.0 } else { 0.0 }
}

#[no_mangle]
pub extern "C" fn Convert_array_to_str(ptr: i64, sep: i64) -> i64 {
    Array_join(ptr, sep)
}

#[no_mangle]
pub extern "C" fn Convert_array_to_map(ptr: i64, kv: i64) -> i64 {
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

#[no_mangle]
pub extern "C" fn Convert_map_to_str(ptr: i64, sep: i64, kv: i64) -> i64 {
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

#[no_mangle]
pub extern "C" fn Convert_map_keys_to_array(ptr: i64) -> i64 {
    Map_keys(ptr)
}

#[no_mangle]
pub extern "C" fn Convert_map_values_to_array(ptr: i64) -> i64 {
    Map_values(ptr)
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.System
// ─────────────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn System_exec(cmd: i64) -> i64 {
    if !is_ptr(cmd) { return unsafe { alloc_str("") }; }
    let cmd_s = unsafe { ptr_to_str(cmd) };
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd_s)
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    let mut out = String::from_utf8_lossy(&output.stdout).to_string();
    if out.ends_with('\n') { out.pop(); }
    unsafe { alloc_str(&out) }
}

#[no_mangle]
pub extern "C" fn System_passthrough(cmd: i64) -> i64 {
    if !is_ptr(cmd) { return 0; }
    let cmd_s = unsafe { ptr_to_str(cmd) };
    let status = Command::new("sh")
        .arg("-c")
        .arg(cmd_s)
        .status()
        .map(|s| s.code().unwrap_or(1))
        .unwrap_or(1);
    status as i64
}

#[no_mangle]
pub extern "C" fn System_exec_code(cmd: i64) -> i64 {
    System_passthrough(cmd)
}

#[no_mangle]
pub extern "C" fn System_exit(code: i64) {
    std::process::exit(code as i32);
}

#[no_mangle]
pub extern "C" fn System_env(name: i64) -> i64 {
    if !is_ptr(name) { return unsafe { alloc_str("") }; }
    let key = unsafe { ptr_to_str(name) };
    let val = std::env::var(key).unwrap_or_default();
    unsafe { alloc_str(&val) }
}

#[no_mangle]
pub extern "C" fn System_set_env(name: i64, val: i64) {
    if !is_ptr(name) { return; }
    let key = unsafe { ptr_to_str(name) };
    let v   = if is_ptr(val) { unsafe { ptr_to_str(val).to_string() } } else { val.to_string() };
    std::env::set_var(key, v);
}

#[no_mangle]
pub extern "C" fn System_cwd() -> i64 {
    let path = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    unsafe { alloc_str(&path) }
}

#[no_mangle]
pub extern "C" fn System_sleep(ms: i64) {
    std::thread::sleep(Duration::from_millis(ms as u64));
}

#[no_mangle]
pub extern "C" fn System_pid() -> i64 {
    std::process::id() as i64
}

#[no_mangle]
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
// ─────────────────────────────────────────────────────────────────────────────

use regex::Regex as Re;

/// Compile le pattern (i64 ptr → &str) ; retourne None si invalide.
unsafe fn compile_regex(pattern: i64) -> Option<Re> {
    let pat = ptr_to_str(pattern);
    Re::new(pat).ok()
}

#[no_mangle]
pub extern "C" fn Regex_test(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = match compile_regex(pattern) { Some(r) => r, None => return 0 };
        let s  = ptr_to_str(text);
        if re.is_match(s) { 1 } else { 0 }
    }
}

#[no_mangle]
pub extern "C" fn Regex_find(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = match compile_regex(pattern) { Some(r) => r, None => return alloc_str("") };
        let s  = ptr_to_str(text);
        match re.find(s) {
            Some(m) => alloc_str(m.as_str()),
            None    => alloc_str(""),
        }
    }
}

#[no_mangle]
pub extern "C" fn Regex_find_all(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = match compile_regex(pattern) { Some(r) => r, None => return new_array() };
        let s  = ptr_to_str(text);
        let arr = new_array();
        for m in re.find_iter(s) {
            let ms = alloc_str(m.as_str());
            __array_push(arr, ms);
        }
        arr
    }
}

#[no_mangle]
pub extern "C" fn Regex_replace(pattern: i64, text: i64, repl: i64) -> i64 {
    unsafe {
        let re = match compile_regex(pattern) { Some(r) => r, None => return text };
        let s  = ptr_to_str(text);
        let r  = ptr_to_str(repl);
        let result = re.replacen(s, 1, r).into_owned();
        alloc_str(&result)
    }
}

#[no_mangle]
pub extern "C" fn Regex_replace_all(pattern: i64, text: i64, repl: i64) -> i64 {
    unsafe {
        let re = match compile_regex(pattern) { Some(r) => r, None => return text };
        let s  = ptr_to_str(text);
        let r  = ptr_to_str(repl);
        let result = re.replace_all(s, r).into_owned();
        alloc_str(&result)
    }
}

#[no_mangle]
pub extern "C" fn Regex_split(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = match compile_regex(pattern) { Some(r) => r, None => return new_array() };
        let s  = ptr_to_str(text);
        let arr = new_array();
        for part in re.split(s) {
            let ps = alloc_str(part);
            __array_push(arr, ps);
        }
        arr
    }
}

#[no_mangle]
pub extern "C" fn Regex_count(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = match compile_regex(pattern) { Some(r) => r, None => return 0 };
        let s  = ptr_to_str(text);
        re.find_iter(s).count() as i64
    }
}

#[no_mangle]
pub extern "C" fn Regex_extract(pattern: i64, text: i64, n: i64) -> i64 {
    unsafe {
        let re = match compile_regex(pattern) { Some(r) => r, None => return alloc_str("") };
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
// ocara.HTTPRequest — implémenté dans http.rs
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

fn ut_fail(msg: &str) {
    let s = format!("FAIL {}\n", msg);
    write_stdout_raw(s.as_bytes());
}

unsafe fn ut_val_to_display(val: i64) -> String {
    // Heuristique simple : si ressemble à un pointeur de chaîne, l'afficher
    if val == 0 {
        return "null".to_string();
    }
    // Booléen boxé (tag 10 en bits bas) ou float (tag 01) → afficher directement
    let tag = val & 0b11;
    if tag == 0b01 {
        // float boxé
        let bits = ((val >> 2) << 2) as u64;
        let f = f64::from_bits(bits);
        return format!("{}", f);
    }
    if tag == 0b10 {
        // bool boxé
        return if (val >> 2) != 0 { "true".to_string() } else { "false".to_string() };
    }
    // Int ou ptr
    if val >= 0x10000 {
        // Probablement une chaîne
        let s = ptr_to_str(val);
        format!("\"{}\"", s)
    } else {
        format!("{}", val)
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertEquals(expected: i64, actual: i64) {
    if expected == actual {
        ut_pass(&format!("assertEquals: {} == {}", expected, actual));
    } else {
        unsafe {
            ut_fail(&format!("assertEquals: attendu {} mais obtenu {}",
                ut_val_to_display(expected), ut_val_to_display(actual)));
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertNotEquals(expected: i64, actual: i64) {
    if expected != actual {
        ut_pass(&format!("assertNotEquals: {} != {}", expected, actual));
    } else {
        unsafe {
            ut_fail(&format!("assertNotEquals: valeurs égales ({})", ut_val_to_display(actual)));
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertTrue(value: i64) {
    if value != 0 {
        ut_pass("assertTrue");
    } else {
        ut_fail("assertTrue: valeur fausse");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertFalse(value: i64) {
    if value == 0 {
        ut_pass("assertFalse");
    } else {
        ut_fail("assertFalse: valeur vraie");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertNull(value: i64) {
    if value == 0 {
        ut_pass("assertNull");
    } else {
        ut_fail("assertNull: valeur non nulle");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertNotNull(value: i64) {
    if value != 0 {
        ut_pass("assertNotNull");
    } else {
        ut_fail("assertNotNull: valeur nulle");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertGreater(a: i64, b: i64) {
    if a > b {
        ut_pass(&format!("assertGreater: {} > {}", a, b));
    } else {
        ut_fail(&format!("assertGreater: {} n'est pas > {}", a, b));
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertLess(a: i64, b: i64) {
    if a < b {
        ut_pass(&format!("assertLess: {} < {}", a, b));
    } else {
        ut_fail(&format!("assertLess: {} n'est pas < {}", a, b));
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertGreaterOrEquals(a: i64, b: i64) {
    if a >= b {
        ut_pass(&format!("assertGreaterOrEquals: {} >= {}", a, b));
    } else {
        ut_fail(&format!("assertGreaterOrEquals: {} n'est pas >= {}", a, b));
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertLessOrEquals(a: i64, b: i64) {
    if a <= b {
        ut_pass(&format!("assertLessOrEquals: {} <= {}", a, b));
    } else {
        ut_fail(&format!("assertLessOrEquals: {} n'est pas <= {}", a, b));
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertContains(haystack: i64, needle: i64) {
    unsafe {
        let h = ptr_to_str(haystack);
        let n = ptr_to_str(needle);
        if h.contains(n) {
            ut_pass(&format!("assertContains: \"{}\" contient \"{}\"", h, n));
        } else {
            ut_fail(&format!("assertContains: \"{}\" ne contient pas \"{}\"", h, n));
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertEmpty(value: i64) {
    let empty = if value == 0 {
        true
    } else if value >= 0x10000 {
        unsafe { ptr_to_str(value).is_empty() }
    } else {
        value == 0
    };
    if empty {
        ut_pass("assertEmpty");
    } else {
        ut_fail("assertEmpty: valeur non vide");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertNotEmpty(value: i64) {
    let empty = if value == 0 {
        true
    } else if value >= 0x10000 {
        unsafe { ptr_to_str(value).is_empty() }
    } else {
        value == 0
    };
    if !empty {
        ut_pass("assertNotEmpty");
    } else {
        ut_fail("assertNotEmpty: valeur vide");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_fail(message: i64) {
    unsafe {
        let msg = ptr_to_str(message);
        ut_fail(msg);
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_pass(message: i64) {
    unsafe {
        let msg = ptr_to_str(message);
        ut_pass(msg);
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertFunction(value: i64) {
    let is_func = __is_function(value) != 0;
    if is_func {
        ut_pass("assertFunction");
    } else {
        ut_fail("assertFunction: la valeur n'est pas une fonction");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertClass(value: i64) {
    let is_obj = __is_object(value) != 0;
    if is_obj {
        ut_pass("assertClass");
    } else {
        ut_fail("assertClass: la valeur n'est pas une instance de classe");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertEnum(value: i64) {
    // Les enums sont implémentés comme des objets en Ocara
    let is_obj = __is_object(value) != 0;
    if is_obj {
        ut_pass("assertEnum");
    } else {
        ut_fail("assertEnum: la valeur n'est pas un enum");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertMap(value: i64) {
    let is_map = __is_map(value) != 0;
    if is_map {
        ut_pass("assertMap");
    } else {
        ut_fail("assertMap: la valeur n'est pas une map");
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertArray(value: i64) {
    let is_arr = __is_array(value) != 0;
    if is_arr {
        ut_pass("assertArray");
    } else {
        ut_fail("assertArray: la valeur n'est pas un array");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Gestion des erreurs — wrappers Rust côté runtime
// (le mécanisme setjmp/longjmp réel est dans try_impl.c)
// ─────────────────────────────────────────────────────────────────────────────

/// Fail non géré : appelé si `fail` est lancé hors de tout `try`.
/// Imprimé en rouge sur stderr, puis exit(1).
/// En pratique, __ocara_fail dans try_impl.c appelle cette logique en C.
/// Ce symbole Rust sert de fallback / documentation.
#[no_mangle]
pub extern "C" fn __ocara_unhandled_fail(val: i64) {
    let msg = format!("\x1b[31mfail non géré: {}\x1b[0m\n", fmt_val(val));
    write_stderr_raw(msg.as_bytes());
    std::process::exit(1);
}

/// Alloue `size` octets sans tag — pour les closures env et allocations internes.
#[no_mangle]
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
/// Le pointeur retourné pointe APRÈS le header de 8 octets.
#[no_mangle]
pub extern "C" fn __alloc_class_obj(size: i64) -> i64 {
    if size <= 0 { return 0; }
    unsafe {
        let total = (size as usize) + 8;
        let layout = Layout::from_size_align(total, 8).unwrap();
        let raw = alloc_zeroed(layout);
        assert!(!raw.is_null(), "ocara_runtime: OOM in __alloc_class_obj");
        *(raw as *mut i64) = TAG_OBJECT;
        (raw as i64) + 8
    }
}

/// Alloue un fat pointer (Function) avec tag TAG_FUNCTION.
/// 16 octets de données : {func_ptr: i64, env_ptr: i64}.
/// Le pointeur retourné pointe APRÈS le header de 8 octets.
#[no_mangle]
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

extern "C" {
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

#[no_mangle]
pub extern "C" fn __ocara_try_exec(body_fn: i64, handler_fn: i64) {
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
        }
    });
}

#[no_mangle]
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
        let msg = format!("\x1b[31mfail non géré: {}\x1b[0m\n", fmt_val(val));
        write_stderr_raw(msg.as_bytes());
        std::process::exit(1);
    }
}

#[no_mangle]
pub extern "C" fn __ocara_type_matches(stored: i64, filter: i64) -> i64 {
    if filter == 0 { return 1; } // pas de filtre → accepte tout
    if stored == 0 { return 0; } // pas de type stocké → ne correspond pas
    unsafe {
        let s = ptr_to_str(stored);
        let f = ptr_to_str(filter);
        if s == f { 1 } else { 0 }
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
#[no_mangle]
pub extern "C" fn __unbox_float(tagged: i64) -> f64 {
    let ptr = (tagged & !1) as *const f64;
    unsafe { *ptr }
}

/// Déboxe un bool précédemment boxé par `__box_bool`.
/// Entrée : `ptr | 2` (tagged heap pointer).
/// Sortie : 0 ou 1 comme i64.
#[no_mangle]
pub extern "C" fn __unbox_bool(tagged: i64) -> i64 {
    let ptr = (tagged & !3) as *const i64;
    unsafe { *ptr }
}

#[no_mangle]
pub extern "C" fn __task_spawn(func: i64, env: i64) -> i64 {
    let handle = std::thread::spawn(move || unsafe {
        let f: extern "C" fn(i64) -> i64 = std::mem::transmute(func as usize);
        f(env)
    });
    let task = Box::new(OcaraTask { handle: Some(handle) });
    Box::into_raw(task) as i64
}

#[no_mangle]
pub extern "C" fn __task_resolve(task_ptr: i64) -> i64 {
    let task = unsafe { &mut *(task_ptr as *mut OcaraTask) };
    if let Some(handle) = task.handle.take() {
        handle.join().unwrap_or(0)
    } else {
        0
    }
}
