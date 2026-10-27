//! Socle des builtins `ocara.Android*` : appels du runtime Ocara (`ffi`),
//! appels des classes Kotlin `com.ocara.bridge.*` (`java`), erreurs levées
//! en `AndroidException` (`Failure`).

pub mod ffi;
pub mod java;

pub const ERR_UNAVAILABLE: i64 = 100;
pub const ERR_PERMISSION: i64 = 101;
pub const ERR_ARGUMENT: i64 = 102;
pub const ERR_DEVICE: i64 = 103;
pub const ERR_IO: i64 = 104;
pub const ERR_ANDROID: i64 = 105;
pub const ERR_TIMEOUT: i64 = 106;
pub const ERR_STATE: i64 = 107;

/// Erreur d'un builtin, levée en `AndroidException` hors de tout appel JNI.
pub struct Failure {
    pub code: i64,
    pub message: String,
}

impl Failure {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    /// Lève `self` ; `source` est le nom du builtin (`AndroidCamera`…).
    pub fn raise(self, source: &'static str) -> ! {
        ffi::raise("AndroidException", source, self.message, self.code)
    }
}
