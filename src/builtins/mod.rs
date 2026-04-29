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
pub mod mutex;
pub mod datetime;
pub mod date;
pub mod time;
pub mod unittest;
pub mod htmlcomponent;
pub mod html;
pub mod file;
pub mod directory;
pub mod exception;

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
        "Mutex"       => Some(mutex::class()),
        "DateTime"    => Some(datetime::class()),
        "Date"        => Some(date::class()),
        "Time"        => Some(time::class()),
        "UnitTest"    => Some(unittest::class()),
        "HTMLComponent" => Some(htmlcomponent::class()),
        "HTML"        => Some(html::class()),
        "File"        => Some(file::class()),
        "Directory"   => Some(directory::class()),
        "Exception"   => Some(exception::exception_class()),
        "FileException" => Some(exception::file_exception_class()),
        "DirectoryException" => Some(exception::directory_exception_class()),
        "IOException" => Some(exception::io_exception_class()),
        "SystemException" => Some(exception::system_exception_class()),
        "ArrayException" => Some(exception::array_exception_class()),
        "MapException" => Some(exception::map_exception_class()),
        "StringException" => Some(exception::string_exception_class()),
        "MathException" => Some(exception::math_exception_class()),
        "ConvertException" => Some(exception::convert_exception_class()),
        "RegexException" => Some(exception::regex_exception_class()),
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
        ("Mutex",       mutex::class()),
        ("DateTime",    datetime::class()),
        ("Date",        date::class()),
        ("Time",        time::class()),
        ("UnitTest",    unittest::class()),
        ("HTMLComponent", htmlcomponent::class()),
        ("HTML",        html::class()),
        ("File",        file::class()),
        ("Directory",   directory::class()),
        ("Exception",   exception::exception_class()),
        ("FileException", exception::file_exception_class()),
        ("DirectoryException", exception::directory_exception_class()),
        ("IOException", exception::io_exception_class()),
        ("SystemException", exception::system_exception_class()),
        ("ArrayException", exception::array_exception_class()),
        ("MapException", exception::map_exception_class()),
        ("StringException", exception::string_exception_class()),
        ("MathException", exception::math_exception_class()),
        ("ConvertException", exception::convert_exception_class()),
        ("RegexException", exception::regex_exception_class()),
    ]
}
