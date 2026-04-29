// ─────────────────────────────────────────────────────────────────────────────
// ocara.Exception — classe d'exception générique
// ocara.FileException — classe d'exception pour les erreurs de fichiers  
// ocara.DirectoryException — classe d'exception pour les erreurs de répertoires
//
// Toutes ont les mêmes propriétés :
//   - message: string  — Description de l'erreur
//   - code: int        — Code d'erreur optionnel (0 = pas de code)
//   - source: string   — Origine de l'erreur ("File", "Directory", etc.)
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
//   } on e is FileException {
//       IO::writeln(`Erreur fichier: ${e.message}`)
//   } on e is DirectoryException {
//       IO::writeln(`Erreur répertoire: ${e.message}`)
//   }
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::{Type, Visibility};
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
