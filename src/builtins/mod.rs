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

pub mod io;
pub mod system;
pub mod thread;
pub mod mutex;
pub mod math;
pub mod string;
pub mod array;
pub mod map;
pub mod regex;
pub mod convert;
pub mod datetime;
pub mod date;
pub mod time;
pub mod file;
pub mod directory;
pub mod json;
pub mod httprequest;
pub mod httpserver;
pub mod htmlcomponent;
pub mod html;
pub mod exception;
pub mod unittest;

use crate::sema::symbols::ClassInfo;

/// Retourne `Some(ClassInfo)` si `name` est une classe builtin du namespace `ocara`.
pub fn builtin_class(name: &str) -> Option<ClassInfo> {
    match name {
        "IO"          => Some(io::class()),
        "System"      => Some(system::class()),
        "Thread"      => Some(thread::class()),
        "Mutex"       => Some(mutex::class()),
        "Math"        => Some(math::class()),
        "String"      => Some(string::class()),
        "Array"       => Some(array::class()),
        "Map"         => Some(map::class()),
        "Regex"       => Some(regex::class()),
        "Convert"     => Some(convert::class()),
        "DateTime"    => Some(datetime::class()),
        "Date"        => Some(date::class()),
        "Time"        => Some(time::class()),
        "File"        => Some(file::class()),
        "Directory"   => Some(directory::class()),
        "JSON"        => Some(json::class()),
        "HTTPRequest" => Some(httprequest::class()),
        "HTTPServer" => Some(httpserver::class()),
        "HTML"        => Some(html::class()),
        "HTMLComponent" => Some(htmlcomponent::class()),
        "Exception"   => Some(exception::exception_class()),
        "FileException" => Some(exception::file_exception_class()),
        "DirectoryException" => Some(exception::directory_exception_class()),
        "IOException" => Some(exception::io_exception_class()),
        "SystemException" => Some(exception::system_exception_class()),
        "ArrayException" => Some(exception::array_exception_class()),
        "MapException" => Some(exception::map_exception_class()),
        "MathException" => Some(exception::math_exception_class()),
        "ConvertException" => Some(exception::convert_exception_class()),
        "RegexException" => Some(exception::regex_exception_class()),
        "DateTimeException" => Some(exception::datetime_exception_class()),
        "DateException" => Some(exception::date_exception_class()),
        "TimeException" => Some(exception::time_exception_class()),
        "ThreadException" => Some(exception::thread_exception_class()),
        "MutexException" => Some(exception::mutex_exception_class()),
        "UnitTestException" => Some(exception::unittest_exception_class()),
        "UnitTest"    => Some(unittest::class()),
        _             => None,
    }
}

/// Toutes les classes builtins (pour `import ocara.*`).
pub fn all_builtins() -> Vec<(&'static str, ClassInfo)> {
    vec![
        ("IO",          io::class()),
        ("System",      system::class()),
        ("Thread",      thread::class()),
        ("Mutex",       mutex::class()),
        ("Math",        math::class()),
        ("String",      string::class()),
        ("Array",       array::class()),
        ("Map",         map::class()),
        ("Regex",       regex::class()),
        ("Convert",     convert::class()),
        ("DateTime",    datetime::class()),
        ("Date",        date::class()),
        ("Time",        time::class()),
        ("File",        file::class()),
        ("Directory",   directory::class()),
        ("JSON",        json::class()),
        ("HTTPRequest", httprequest::class()),
        ("HTTPServer", httpserver::class()),
        ("HTML",        html::class()),
        ("HTMLComponent", htmlcomponent::class()),
        ("Exception",   exception::exception_class()),
        ("FileException", exception::file_exception_class()),
        ("DirectoryException", exception::directory_exception_class()),
        ("IOException", exception::io_exception_class()),
        ("SystemException", exception::system_exception_class()),
        ("ArrayException", exception::array_exception_class()),
        ("MapException", exception::map_exception_class()),
        ("MathException", exception::math_exception_class()),
        ("ConvertException", exception::convert_exception_class()),
        ("RegexException", exception::regex_exception_class()),
        ("DateTimeException", exception::datetime_exception_class()),
        ("DateException", exception::date_exception_class()),
        ("TimeException", exception::time_exception_class()),
        ("ThreadException", exception::thread_exception_class()),
        ("MutexException", exception::mutex_exception_class()),
        ("UnitTestException", exception::unittest_exception_class()),
        ("UnitTest",    unittest::class()),
    ]
}
