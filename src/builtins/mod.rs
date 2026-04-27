// ─────────────────────────────────────────────────────────────────────────────
// src/builtins/mod.rs
//
// Registre central des classes builtins du namespace `ocara`.
//
// Pour ajouter une nouvelle classe :
//   1. Créer src/builtins/<nom>.rs
//   2. Ajouter `pub mod <nom>;` ici
//   3. Référencer dans `builtin_class()` et `all_builtins()`
//   4. Ajouter les sigs Cranelift dans `src/codegen/runtime.rs`
// ─────────────────────────────────────────────────────────────────────────────

pub mod array;
pub mod convert;
pub mod http;
pub mod io;
pub mod map;
pub mod math;
pub mod regex;
pub mod string;
pub mod system;
pub mod httpserver;
pub mod thread;
pub mod unittest;

use crate::sema::symbols::ClassInfo;

/// Retourne `Some(ClassInfo)` si `name` est une classe builtin du namespace `ocara`.
pub fn builtin_class(name: &str) -> Option<ClassInfo> {
    match name {
        "Array"       => Some(array::class()),
        "Convert"     => Some(convert::class()),
        "HTTPRequest" => Some(http::class()),
        "IO"          => Some(io::class()),
        "Map"         => Some(map::class()),
        "Math"        => Some(math::class()),
        "Regex"       => Some(regex::class()),
        "String"      => Some(string::class()),
        "System"      => Some(system::class()),
        "HTTPServer" => Some(httpserver::class()),
        "Thread"      => Some(thread::class()),
        "UnitTest"    => Some(unittest::class()),
        _             => None,
    }
}

/// Toutes les classes builtins (pour `import ocara.*`).
pub fn all_builtins() -> Vec<(&'static str, ClassInfo)> {
    vec![
        ("Array",       array::class()),
        ("Convert",     convert::class()),
        ("HTTPRequest", http::class()),
        ("IO",          io::class()),
        ("Map",         map::class()),
        ("Math",        math::class()),
        ("Regex",       regex::class()),
        ("String",      string::class()),
        ("System",      system::class()),
        ("HTTPServer", httpserver::class()),
        ("Thread",      thread::class()),
        ("UnitTest",    unittest::class()),
    ]
}
