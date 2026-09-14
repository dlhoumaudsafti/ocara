// ─────────────────────────────────────────────────────────────────────────────
// Exception helpers pour les builtins runtime
//
// Le 2ᵉ argument de `__ocara_fail` est la CHAÎNE D'ANCÊTRES de la classe levée
// (elle-même en premier), jointe par '|' — même convention que `lower_raise`
// (voir IrModule::ancestor_chain, src/lower/stmt.d/statements.d/exceptions.rs)
// : `__ocara_type_matches` (runtime/src/lib.rs) cherche le filtre `on e is X`
// comme un des maillons, pas une égalité stricte, pour qu'`on e is Exception`
// attrape n'importe laquelle de ces exceptions builtin (toutes des sous-
// classes directes et uniques d'`Exception` — hiérarchie plate d'un seul
// niveau, câblée ici en dur plutôt que via un mécanisme générique, voir
// docs/roadmap.d/langage-exceptions.md). Seule `throw_exception` (la base)
// n'a pas de parent, donc pas de suffixe `|Exception`.
// ─────────────────────────────────────────────────────────────────────────────

use std::alloc::{alloc, Layout};
use crate::{alloc_str, __ocara_fail};
use crate::typecheck::TAG_EXCEPTION;

/// Structure runtime pour Exception / FileException / DirectoryException / IOException / SystemException / ArrayException / MapException / MathException / ConvertException / RegexException / HTTPServerException
/// { message: string, code: int, source: string }
#[repr(C)]
struct OcaraException {
    message: i64,  // pointeur vers string
    code: i64,     // entier
    source: i64,   // pointeur vers string
}

/// Crée une Exception générique et la lève (ne retourne jamais)
pub unsafe fn throw_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Crée une FileException et la lève (ne retourne jamais)
pub unsafe fn throw_file_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("FileException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Crée une DirectoryException et la lève (ne retourne jamais)
pub unsafe fn throw_directory_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("DirectoryException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Crée une IOException et la lève (ne retourne jamais)
pub unsafe fn throw_io_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("IOException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Crée une SystemException et la lève (ne retourne jamais)
pub unsafe fn throw_system_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("SystemException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Crée une ArrayException et la lève (ne retourne jamais)
pub unsafe fn throw_array_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("ArrayException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Crée une MapException et la lève (ne retourne jamais)
pub unsafe fn throw_map_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("MapException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Crée une MathException et la lève (ne retourne jamais)
pub unsafe fn throw_math_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("MathException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Crée une ConvertException et la lève (ne retourne jamais)
pub unsafe fn throw_convert_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("ConvertException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Crée une RegexException et la lève (ne retourne jamais)
pub unsafe fn throw_regex_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("RegexException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Lance une DateTimeException
pub unsafe fn throw_datetime_exception(message: &str, code: i64) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, "DateTime");
        let type_name = alloc_str("DateTimeException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Lance une DateException
pub unsafe fn throw_date_exception(message: &str, code: i64) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, "Date");
        let type_name = alloc_str("DateException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Lance une TimeException
pub unsafe fn throw_time_exception(message: &str, code: i64) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, "Time");
        let type_name = alloc_str("TimeException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Lance une ThreadException
pub unsafe fn throw_thread_exception(message: &str, code: i64) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, "Thread");
        let type_name = alloc_str("ThreadException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Lance une MutexException
pub unsafe fn throw_mutex_exception(message: &str, code: i64) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, "Mutex");
        let type_name = alloc_str("MutexException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Lance une UnitTestException
pub unsafe fn throw_unittest_exception(message: &str, code: i64) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, "UnitTest");
        let type_name = alloc_str("UnitTestException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}

/// Lance une HTTPServerException
pub unsafe fn throw_httpserver_exception(message: &str, code: i64) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, "HTTPServer");
        let type_name = alloc_str("HTTPServerException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}
/// Lance une TauriException
pub unsafe fn throw_tauri_exception(message: &str, code: i64) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, "Tauri");
        let type_name = alloc_str("TauriException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}
/// Lance une SDLException
pub unsafe fn throw_sdl_exception(message: &str, code: i64) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, "SDL");
        let type_name = alloc_str("SDLException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}
/// Lance une SQLiteException
pub unsafe fn throw_sqlite_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("SQLiteException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}
/// Lance une MySQLException (également utilisée par l'alias MariaDB)
pub unsafe fn throw_mysql_exception(message: &str, code: i64, source: &str) -> ! {
    unsafe {
        let obj_ptr = alloc_exception(message, code, source);
        let type_name = alloc_str("MySQLException|Exception");
        __ocara_fail(obj_ptr, type_name);
        std::hint::unreachable_unchecked()
    }
}
/// Alloue un objet Exception sur le heap
unsafe fn alloc_exception(message: &str, code: i64, source: &str) -> i64 {
    unsafe {
        let size = std::mem::size_of::<OcaraException>();
        let layout = Layout::from_size_align(8 + size, 8).unwrap();
        let raw = alloc(layout);
        assert!(!raw.is_null(), "ocara_runtime: OOM (exception)");
        
        // Tag dédié aux exceptions — voir TAG_EXCEPTION pour la confusion
        // avec TAG_MAP que ce tag corrige.
        *(raw as *mut i64) = TAG_EXCEPTION;
        
        // Objet Exception
        let exc_ptr = raw.add(8) as *mut OcaraException;
        std::ptr::write(exc_ptr, OcaraException {
            message: alloc_str(message),
            code,
            source: alloc_str(source),
        });
        
        (raw as i64) + 8
    }
}
