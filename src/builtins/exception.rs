// ─────────────────────────────────────────────────────────────────────────────
// ocara.Exception — classe d'exception générique
// ocara.FileException — classe d'exception pour les erreurs de fichiers  
// ocara.DirectoryException — classe d'exception pour les erreurs de répertoires
// ocara.IOException — classe d'exception pour les erreurs d'entrées/sorties
// ocara.SystemException — classe d'exception pour les erreurs système
// ocara.ArrayException — classe d'exception pour les erreurs de tableaux
// ocara.MapException — classe d'exception pour les erreurs de maps
// ocara.MathException — classe d'exception pour les erreurs mathématiques
// ocara.ConvertException — classe d'exception pour les erreurs de conversion
// ocara.RegexException — classe d'exception pour les erreurs d'expressions régulières
// ocara.DateTimeException — classe d'exception pour les erreurs de date/heure
// ocara.DateException — classe d'exception pour les erreurs de date
// ocara.TimeException — classe d'exception pour les erreurs de temps
// ocara.ThreadException — classe d'exception pour les erreurs de threads
// ocara.MutexException — classe d'exception pour les erreurs de mutex
// ocara.UnitTestException — classe d'exception pour les échecs d'assertions de tests
//
// Toutes ont les mêmes propriétés :
//   - message: string  — Description de l'erreur
//   - code: int        — Code d'erreur optionnel (0 = pas de code)
//   - source: string   — Origine de l'erreur ("File", "Directory", "IO", "System", "Array", "Map", "String", "Math", "Convert", "Regex", etc.)
//
// Usage générique (attrape tout) :
//   try {
//       File::read("/inexistant.txt")
//   } on e {
//       IO::writeln(`Erreur: ${e.message}`)
//   }
//
// Usage ciblé (filtrage par type) :
//   try {
//       File::read("/a.txt")
//       Directory::create("/b")
//       var input:string = IO::read()
//       var out:string = System::exec("invalid-cmd")
//       var arr:Array<int> = Array::new()
//       var val:int = arr.pop()
//       var result:float = Math::sqrt(-1.0)
//       var num:int = Convert::str_to_int("abc")
//   } on e is FileException {
//       IO::writeln(`Erreur fichier: ${e.message}`)
//   } on e is DirectoryException {
//       IO::writeln(`Erreur répertoire: ${e.message}`)
//   } on e is IOException {
//       IO::writeln(`Erreur IO: ${e.message}`)
//   } on e is SystemException {
//       IO::writeln(`Erreur système: ${e.message}`)
//   } on e is ArrayException {
//       IO::writeln(`Erreur tableau: ${e.message}`)
//   } on e is MapException {
//       IO::writeln(`Erreur map: ${e.message}`)
//   } on e is MathException {
//       IO::writeln(`Erreur mathématique: ${e.message}`)
//   } on e is ConvertException {
//       IO::writeln(`Erreur de conversion: ${e.message}`)
//   } on e is RegexException {
//       IO::writeln(`Erreur regex: ${e.message}`)
//   } on e is DateTimeException {
//       IO::writeln(`Erreur date/heure: ${e.message}`)
//   } on e is DateException {
//       IO::writeln(`Erreur date: ${e.message}`)
//   } on e is TimeException {
//       IO::writeln(`Erreur temps: ${e.message}`)
//   } on e is ThreadException {
//       IO::writeln(`Erreur thread: ${e.message}`)
//   } on e is MutexException {
//       IO::writeln(`Erreur mutex: ${e.message}`)
//   } on e is UnitTestException {
//       IO::writeln(`Échec de test: ${e.message}`)
//   }
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::{Type, Visibility};
use crate::sema::symbols::{ClassInfo, FieldInfo};

fn make_exception_class() -> ClassInfo {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), FieldInfo {
        ty: Type::String,
        mutable: false,
        vis: Visibility::Public,
    });
    fields.insert("code".to_string(), FieldInfo {
        ty: Type::Int,
        mutable: false,
        vis: Visibility::Public,
    });
    fields.insert("source".to_string(), FieldInfo {
        ty: Type::String,
        mutable: false,
        vis: Visibility::Public,
    });

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields,
        methods:      HashMap::new(),
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}

pub fn exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn file_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn directory_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn io_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn system_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn array_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn map_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn math_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn convert_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn regex_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn datetime_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn date_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn time_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn thread_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn mutex_exception_class() -> ClassInfo {
    make_exception_class()
}

pub fn unittest_exception_class() -> ClassInfo {
    make_exception_class()
}
