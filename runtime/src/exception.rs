// ─────────────────────────────────────────────────────────────────────────────
// Exception helpers pour les builtins runtime
// ─────────────────────────────────────────────────────────────────────────────

use std::alloc::{alloc, Layout};
use crate::{alloc_str, __ocara_fail};

/// Structure runtime pour Exception / FileException / DirectoryException / IOException
/// { message: string, code: int, source: string }
#[repr(C)]
struct OcaraException {
    message: i64,  // pointeur vers string
    code: i64,     // entier
    source: i64,   // pointeur vers string
}

/// Crée une Exception générique et la lève (ne retourne jamais)
pub unsafe fn throw_exception(message: &str, code: i64, source: &str) -> ! {
    let obj_ptr = alloc_exception(message, code, source);
    let type_name = alloc_str("Exception");
    __ocara_fail(obj_ptr, type_name);
    std::hint::unreachable_unchecked()
}

/// Crée une FileException et la lève (ne retourne jamais)
pub unsafe fn throw_file_exception(message: &str, code: i64, source: &str) -> ! {
    let obj_ptr = alloc_exception(message, code, source);
    let type_name = alloc_str("FileException");
    __ocara_fail(obj_ptr, type_name);
    std::hint::unreachable_unchecked()
}

/// Crée une DirectoryException et la lève (ne retourne jamais)
pub unsafe fn throw_directory_exception(message: &str, code: i64, source: &str) -> ! {
    let obj_ptr = alloc_exception(message, code, source);
    let type_name = alloc_str("DirectoryException");
    __ocara_fail(obj_ptr, type_name);
    std::hint::unreachable_unchecked()
}

/// Crée une IOException et la lève (ne retourne jamais)
pub unsafe fn throw_io_exception(message: &str, code: i64, source: &str) -> ! {
    let obj_ptr = alloc_exception(message, code, source);
    let type_name = alloc_str("IOException");
    __ocara_fail(obj_ptr, type_name);
    std::hint::unreachable_unchecked()
}

/// Alloue un objet Exception sur le heap
unsafe fn alloc_exception(message: &str, code: i64, source: &str) -> i64 {
    let size = std::mem::size_of::<OcaraException>();
    let layout = Layout::from_size_align(8 + size, 8).unwrap();
    let raw = alloc(layout);
    assert!(!raw.is_null(), "ocara_runtime: OOM (exception)");
    
    // Tag optionnel (0x03 pour Exception)
    *(raw as *mut i64) = 0x0000_0000_0000_0003;
    
    // Objet Exception
    let exc_ptr = raw.add(8) as *mut OcaraException;
    std::ptr::write(exc_ptr, OcaraException {
        message: alloc_str(message),
        code,
        source: alloc_str(source),
    });
    
    (raw as i64) + 8
}
