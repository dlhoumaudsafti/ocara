//! Builtins bloquants dans un binaire de test ocaraunit.
//!
//! ocaraunit lance ses binaires avec `OCARA_UNITTEST` dans l'environnement.
//! Un builtin qui ne rend jamais la main de lui-même (`Tauri.run`,
//! `HTTPServer.run`) appelé sur le thread principal rend alors la main tout
//! de suite : la ligne est exécutée, le flot va jusqu'au bout, et l'appel
//! est signalé par une ligne `BLOCKED <builtin>` que relève ocaraunit. Sur
//! un autre thread (serveur lancé dans un `Thread`), l'appel s'exécute
//! réellement. Étiquettes côté compilateur : `src/builtins/blocking.rs`.

use std::sync::OnceLock;

static TEST_MODE: OnceLock<bool> = OnceLock::new();

/// Binaire lancé par ocaraunit.
pub fn test_mode() -> bool {
    *TEST_MODE.get_or_init(|| std::env::var_os("OCARA_UNITTEST").is_some())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn on_main_thread() -> bool {
    unsafe { libc::syscall(libc::SYS_gettid) == libc::getpid() as libc::c_long }
}

#[cfg(target_os = "macos")]
fn on_main_thread() -> bool {
    unsafe { libc::pthread_main_np() != 0 }
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
fn on_main_thread() -> bool {
    true
}

/// Vrai si l'appel bloquant `builtin` doit rendre la main immédiatement
/// (binaire de test, thread principal) ; l'appel est alors signalé.
pub fn neutralized(builtin: &str) -> bool {
    if !test_mode() || !on_main_thread() {
        return false;
    }
    crate::write_stdout_raw(format!("BLOCKED {}\n", builtin).as_bytes());
    true
}
