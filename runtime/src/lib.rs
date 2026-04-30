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
//   comme des pointeurs. Pour ces cas, utiliser Convert::intToStr() d'abord.
// ─────────────────────────────────────────────────────────────────────────────

#![allow(clippy::missing_safety_doc)]

use std::alloc::{alloc, alloc_zeroed, Layout};
use crate::typecheck::{TAG_STRING, TAG_ARRAY, TAG_MAP, TAG_OBJECT, TAG_FUNCTION, 
    __is_function, __is_object, __is_map, __is_array, __is_string};
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
pub mod file;
pub mod directory;
pub mod exception;

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

/// Fonction interne (pas exportée en C) — écrit une valeur sur stdout.
/// NOTE : pas de #[no_mangle] pour éviter de shadower le `write(fd,buf,n)` POSIX
///        dont Rust's std::fs::write a besoin en interne.
#[allow(dead_code)]
fn write(val: i64) {
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
//
// Codes d'erreur IOException :
//   101 - READ  : Erreur de lecture depuis stdin
//   102 - WRITE : Erreur d'écriture sur stdout
// ─────────────────────────────────────────────────────────────────────────────

const ERR_IO_READ: i64 = 101;
#[allow(dead_code)]  // Réservé pour une future implémentation d'erreurs write
const ERR_IO_WRITE: i64 = 102;

#[no_mangle]
pub extern "C" fn IO_write(val: i64) {
    ocara_print(&fmt_val(val));
}

#[no_mangle]
pub extern "C" fn IO_writeInt(n: i64) {
    ocara_print(&n.to_string());
}

#[no_mangle]
pub extern "C" fn IO_writeFloat(f: f64) {
    ocara_print(&f.to_string());
}

#[no_mangle]
pub extern "C" fn IO_writeBool(b: i64) {
    ocara_print(if b != 0 { "true" } else { "false" });
}

#[no_mangle]
pub extern "C" fn IO_writeln(val: i64) {
    ocara_println(&fmt_val(val));
}

#[no_mangle]
pub extern "C" fn IO_writelnInt(n: i64) {
    ocara_println(&n.to_string());
}

#[no_mangle]
pub extern "C" fn IO_writelnFloat(f: f64) {
    ocara_println(&f.to_string());
}

#[no_mangle]
pub extern "C" fn IO_writelnBool(b: i64) {
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
pub extern "C" fn IO_readInt() -> i64 {
    let s = read();
    if s == 0 { return 0; }
    unsafe { ptr_to_str(s).trim().parse::<i64>().unwrap_or(0) }
}

#[no_mangle]
pub extern "C" fn IO_readFloat() -> f64 {
    let s = read();
    if s == 0 { return 0.0; }
    unsafe { ptr_to_str(s).trim().parse::<f64>().unwrap_or(0.0) }
}

#[no_mangle]
pub extern "C" fn IO_readBool() -> i64 {
    let s = read();
    if s == 0 { return 0; }
    let t = unsafe { ptr_to_str(s).trim().to_lowercase() };
    if t == "true" || t == "1" { 1 } else { 0 }
}

#[no_mangle]
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

#[no_mangle]
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
//
// Codes d'erreur MathException :
//   101 - NEGATIVE_SQRT : Racine carrée d'un nombre négatif
//   102 - NEGATIVE_EXPONENT : Exposant négatif dans pow()
// ─────────────────────────────────────────────────────────────────────────────

const ERR_MATH_NEGATIVE_SQRT: i64 = 101;
const ERR_MATH_NEGATIVE_EXPONENT: i64 = 102;

#[no_mangle]
pub extern "C" fn Math_abs(n: i64) -> i64 { n.abs() }

#[no_mangle]
pub extern "C" fn Math_min(a: i64, b: i64) -> i64 { a.min(b) }

#[no_mangle]
pub extern "C" fn Math_max(a: i64, b: i64) -> i64 { a.max(b) }

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Math_clamp(n: i64, lo: i64, hi: i64) -> i64 { n.clamp(lo, hi) }

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Math_floor(n: f64) -> i64 { n.floor() as i64 }

#[no_mangle]
pub extern "C" fn Math_ceil(n: f64) -> i64 { n.ceil() as i64 }

#[no_mangle]
pub extern "C" fn Math_round(n: f64) -> i64 { n.round() as i64 }

// ─────────────────────────────────────────────────────────────────────────────
// ocara.Array
//
// Codes d'erreur ArrayException :
//   101 - EMPTY_ARRAY : Opération sur un tableau vide (pop, first, last)
// ─────────────────────────────────────────────────────────────────────────────

const ERR_ARRAY_EMPTY: i64 = 101;

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

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Array_contains(ptr: i64, val: i64) -> i64 {
    if ptr == 0 { return 0; }
    if unsafe { array_ref(ptr).data.contains(&val) } { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Array_indexOf(ptr: i64, val: i64) -> i64 {
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

#[no_mangle]
pub extern "C" fn Array_get(ptr: i64, idx: i64) -> i64 {
    __array_get(ptr, idx)
}

#[no_mangle]
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
pub extern "C" fn Map_isEmpty(ptr: i64) -> i64 {
    if ptr == 0 { return 1; }
    if unsafe { map_ref(ptr).data.is_empty() } { 1 } else { 0 }
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

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Convert_strToBool(s: i64) -> i64 {
    if !is_ptr(s) { return 0; }
    let t = unsafe { ptr_to_str(s).trim().to_lowercase() };
    if t == "true" || t == "1" { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Convert_strToArray(s: i64, sep: i64) -> i64 {
    String_split(s, sep)
}

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Convert_intToStr(n: i64) -> i64 {
    unsafe { alloc_str(&n.to_string()) }
}

#[no_mangle]
pub extern "C" fn Convert_intToFloat(n: i64) -> f64 {
    n as f64
}

#[no_mangle]
pub extern "C" fn Convert_intToBool(n: i64) -> i64 {
    if n != 0 { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Convert_floatToStr(f: f64) -> i64 {
    unsafe { alloc_str(&f.to_string()) }
}

#[no_mangle]
pub extern "C" fn Convert_floatToInt(f: f64) -> i64 {
    f as i64
}

#[no_mangle]
pub extern "C" fn Convert_floatToBool(f: f64) -> i64 {
    if f != 0.0 { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Convert_boolToStr(b: i64) -> i64 {
    unsafe { alloc_str(if b != 0 { "true" } else { "false" }) }
}

#[no_mangle]
pub extern "C" fn Convert_boolToInt(b: i64) -> i64 {
    if b != 0 { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn Convert_boolToFloat(b: i64) -> f64 {
    if b != 0 { 1.0 } else { 0.0 }
}

#[no_mangle]
pub extern "C" fn Convert_arrayToStr(ptr: i64, sep: i64) -> i64 {
    Array_join(ptr, sep)
}

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Convert_mapKeysToArray(ptr: i64) -> i64 {
    Map_keys(ptr)
}

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn System_execCode(cmd: i64) -> i64 {
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
pub extern "C" fn System_setEnv(name: i64, val: i64) {
    if !is_ptr(name) { return; }
    let key = unsafe { ptr_to_str(name) };
    let v   = if is_ptr(val) { unsafe { ptr_to_str(val).to_string() } } else { val.to_string() };
    
    // set_var peut paniquer si le nom ou la valeur contient '=' ou NUL
    // On catch le panic potentiel
    match std::panic::catch_unwind(|| {
        std::env::set_var(key, &v);
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

#[no_mangle]
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
//
// Codes d'erreur RegexException :
//   101 - INVALID_PATTERN : Pattern regex invalide (erreur de syntaxe)
// ─────────────────────────────────────────────────────────────────────────────

use regex::Regex as Re;

const ERR_REGEX_INVALID_PATTERN: i64 = 101;

/// Compile le pattern (i64 ptr → &str) ; lève RegexException si invalide.
unsafe fn compile_regex(pattern: i64) -> Re {
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

#[no_mangle]
pub extern "C" fn Regex_test(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        if re.is_match(s) { 1 } else { 0 }
    }
}

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Regex_replace(pattern: i64, text: i64, repl: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        let r  = ptr_to_str(repl);
        let result = re.replacen(s, 1, r).into_owned();
        alloc_str(&result)
    }
}

#[no_mangle]
pub extern "C" fn Regex_replaceAll(pattern: i64, text: i64, repl: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        let r  = ptr_to_str(repl);
        let result = re.replace_all(s, r).into_owned();
        alloc_str(&result)
    }
}

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn Regex_count(pattern: i64, text: i64) -> i64 {
    unsafe {
        let re = compile_regex(pattern);
        let s  = ptr_to_str(text);
        re.find_iter(s).count() as i64
    }
}

#[no_mangle]
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
            crate::exception::throw_unittest_exception(
                &format!("assertEquals: expected {} but got {}",
                    ut_val_to_display(expected), ut_val_to_display(actual)),
                101
            );
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertNotEquals(expected: i64, actual: i64) {
    if expected != actual {
        ut_pass(&format!("assertNotEquals: {} != {}", expected, actual));
    } else {
        unsafe {
            crate::exception::throw_unittest_exception(
                &format!("assertNotEquals: values are equal ({})", ut_val_to_display(actual)),
                102
            );
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertTrue(value: i64) {
    if value != 0 {
        ut_pass("assertTrue");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertTrue: value is false", 103);
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertFalse(value: i64) {
    if value == 0 {
        ut_pass("assertFalse");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertFalse: value is true", 104);
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertNull(value: i64) {
    if value == 0 {
        ut_pass("assertNull");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertNull: value is not null", 105);
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertNotNull(value: i64) {
    if value != 0 {
        ut_pass("assertNotNull");
    } else {
        unsafe {
            crate::exception::throw_unittest_exception("assertNotNull: value is null", 106);
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_assertGreater(a: i64, b: i64) {
    if a > b {
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

#[no_mangle]
pub extern "C" fn UnitTest_assertLess(a: i64, b: i64) {
    if a < b {
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

#[no_mangle]
pub extern "C" fn UnitTest_assertGreaterOrEquals(a: i64, b: i64) {
    if a >= b {
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

#[no_mangle]
pub extern "C" fn UnitTest_assertLessOrEquals(a: i64, b: i64) {
    if a <= b {
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

#[no_mangle]
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
        unsafe {
            crate::exception::throw_unittest_exception("assertEmpty: value is not empty", 112);
        }
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
        unsafe {
            crate::exception::throw_unittest_exception("assertNotEmpty: value is empty", 113);
        }
    }
}

#[no_mangle]
pub extern "C" fn UnitTest_fail(message: i64) {
    unsafe {
        let msg = ptr_to_str(message);
        crate::exception::throw_unittest_exception(msg, 114);
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
        unsafe {
            crate::exception::throw_unittest_exception("assertFunction: value is not a function", 115);
        }
    }
}

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
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

#[no_mangle]
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

/// Egalite stricte (===) : retourne 1 si meme type ET meme valeur, 0 sinon.
#[no_mangle]
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
    
    // Types primitifs (int/float/bool) : comparaison directe
    if lhs_type == 1 {
        return if lhs == rhs { 1 } else { 0 };
    }
    
    // Strings : comparer le contenu
    if lhs_type == 4 {
        return if unsafe { ptr_to_str(lhs) == ptr_to_str(rhs) } { 1 } else { 0 };
    }
    
    // Autres types heap : comparaison de pointeurs
    if lhs == rhs { 1 } else { 0 }
}

/// Inegalite stricte (!==) : retourne 1 si types differents OU valeurs differentes, 0 sinon.
#[no_mangle]
pub extern "C" fn __cmp_ne_strict(lhs: i64, rhs: i64) -> i64 {
    if __cmp_eq_strict(lhs, rhs) != 0 { 0 } else { 1 }
}

/// Inferieur ou egal strict (<==) : retourne 1 si meme type ET lhs <= rhs, 0 sinon.
#[no_mangle]
pub extern "C" fn __cmp_le_strict(lhs: i64, rhs: i64) -> i64 {
    let lhs_type = get_value_type(lhs);
    let rhs_type = get_value_type(rhs);
    
    // Types differents -> false
    if lhs_type != rhs_type {
        return 0;
    }
    
    // Primitifs : comparaison directe (ne distingue pas int/float/bool)
    if lhs_type == 1 {
        return if lhs <= rhs { 1 } else { 0 };
    }
    
    // Autres types non comparables
    0
}

/// Superieur ou egal strict (>==) : retourne 1 si meme type ET lhs >= rhs, 0 sinon.
#[no_mangle]
pub extern "C" fn __cmp_ge_strict(lhs: i64, rhs: i64) -> i64 {
    let lhs_type = get_value_type(lhs);
    let rhs_type = get_value_type(rhs);
    
    // Types differents -> false
    if lhs_type != rhs_type {
        return 0;
    }
    
    // Primitifs : comparaison directe (ne distingue pas int/float/bool)
    if lhs_type == 1 {
        return if lhs >= rhs { 1 } else { 0 };
    }
    
    // Autres types non comparables
    0
}

// ────────────────────────────────────────────────────────────────────────────
// ocara.JSON — Sérialisation et désérialisation JSON
// ────────────────────────────────────────────────────────────────────────────

use serde_json::{Value as JsonValue, Map as JsonMap};

/// JSON::encode(data) → string
/// Encode un array ou map en JSON
#[no_mangle]
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
    
    let typ = get_value_type(val);
    
    match typ {
        1 => {  // Primitif (int ou bool)
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
#[no_mangle]
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
                i
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
#[no_mangle]
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
#[no_mangle]
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
