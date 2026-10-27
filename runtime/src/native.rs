//! Valeurs natives comptées (`TAG_NATIVE`) : une structure d'un crate
//! séparé (`ocara.Image`…) rendue comme valeur Ocara, libérée par le
//! comptage de références comme une chaîne ou un tableau — contrairement à
//! un handle (`rc::handle_new`), jamais compté.
//!
//! Interface en ABI C : un crate séparé qui dépendrait d'`ocara_runtime` en
//! Rust embarquerait sa propre copie du runtime, et sous Windows une partie
//! de ses appels se lierait à cette copie (pile des `try`, caches…) au lieu
//! du runtime du programme.

use crate::rc;
use crate::typecheck::{read_tag, TAG_NATIVE};

/// `[data][drop_fn]`
const PAYLOAD_SIZE: usize = 16;

/// Valeur comptée (compte initial 1) qui possède `data`, détruite par
/// `drop_fn(data)` avec sa dernière référence.
#[unsafe(no_mangle)]
pub extern "C" fn __native_new(data: i64, drop_fn: i64) -> i64 {
    unsafe {
        let val = rc::alloc_block(PAYLOAD_SIZE, 0, TAG_NATIVE, false);
        *(val as *mut i64) = data;
        *((val + 8) as *mut i64) = drop_fn;
        val
    }
}

/// `data` d'une valeur native créée avec `drop_fn` ; 0 pour `null`, une
/// autre valeur, ou une valeur native d'un autre type (autre `drop_fn`).
#[unsafe(no_mangle)]
pub extern "C" fn __native_data(val: i64, drop_fn: i64) -> i64 {
    unsafe {
        if read_tag(val) != TAG_NATIVE || *((val + 8) as *const i64) != drop_fn {
            return 0;
        }
        *(val as *const i64)
    }
}

pub(crate) unsafe fn free_native(val: i64) {
    unsafe {
        let drop_fn: extern "C" fn(i64) = std::mem::transmute(*((val + 8) as *const i64) as usize);
        drop_fn(*(val as *const i64));
        rc::free_block(val, PAYLOAD_SIZE);
    }
}

/// Longueur en octets d'une chaîne Ocara (ses octets commencent à `val`).
#[unsafe(no_mangle)]
pub extern "C" fn __str_byte_len(val: i64) -> i64 {
    if val == 0 { 0 } else { unsafe { crate::str_append::str_len(val) as i64 } }
}

/// Chaîne Ocara possédée, copie des `len` octets UTF-8 de `ptr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __str_from_utf8(ptr: *const u8, len: usize) -> i64 {
    unsafe {
        let bytes = std::slice::from_raw_parts(ptr, len);
        crate::alloc_str(std::str::from_utf8_unchecked(bytes))
    }
}

/// Corps des talons `Android*_*` hors Android (`src/codegen/emit.d/emitter.rs`) :
/// le programme compile partout, l'appel lève à l'exécution.
#[unsafe(no_mangle)]
pub extern "C" fn __android_unavailable() {
    let (class, source) = ("AndroidException", "Android");
    let message = "fonctionnalité disponible uniquement dans un programme compilé pour Android";
    unsafe {
        crate::exception::__raise_builtin_exception(
            class.as_ptr(), class.len(),
            source.as_ptr(), source.len(),
            message.as_ptr(), message.len(),
            100,
        )
    }
}
