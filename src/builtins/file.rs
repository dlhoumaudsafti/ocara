// ─────────────────────────────────────────────────────────────────────────────
// ocara.File — classe builtin statique pour opérations sur fichiers
//
// Méthodes statiques :
//   File::read(path:string|File) → string
//   File::read_bytes(path:string|File) → int[]
//   File::write(path:string|File, content:string) → void
//   File::write_bytes(path:string|File, data:int[]) → void
//   File::append(path:string|File, content:string) → void
//   File::exists(path:string|File) → bool
//   File::size(path:string|File) → int
//   File::extension(path:string|File) → string
//   File::remove(path:string|File) → void
//   File::copy(src:string|File, dst:string|File) → void
//   File::move(src:string|File, dst:string|File) → void
//   File::infos(path:string|File) → map<string, mixed>
//
// Convention runtime : File_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // ── Lecture ───────────────────────────────────────────────────────────────

    // File::read(path:string) → string
    methods.insert("read".into(), m(
        vec![("path", Type::String)],
        Type::String,
    ));

    // File::read_bytes(path:string) → int[]
    methods.insert("read_bytes".into(), m(
        vec![("path", Type::String)],
        Type::Array(Box::new(Type::Int)),
    ));

    // ── Écriture ──────────────────────────────────────────────────────────────

    // File::write(path:string, content:string) → void
    methods.insert("write".into(), m(
        vec![("path", Type::String), ("content", Type::String)],
        Type::Void,
    ));

    // File::write_bytes(path:string, data:int[]) → void
    methods.insert("write_bytes".into(), m(
        vec![("path", Type::String), ("data", Type::Array(Box::new(Type::Int)))],
        Type::Void,
    ));

    // File::append(path:string, content:string) → void
    methods.insert("append".into(), m(
        vec![("path", Type::String), ("content", Type::String)],
        Type::Void,
    ));

    // ── Métadonnées ───────────────────────────────────────────────────────────

    // File::exists(path:string) → bool
    methods.insert("exists".into(), m(
        vec![("path", Type::String)],
        Type::Bool,
    ));

    // File::size(path:string) → int
    methods.insert("size".into(), m(
        vec![("path", Type::String)],
        Type::Int,
    ));

    // File::extension(path:string) → string
    methods.insert("extension".into(), m(
        vec![("path", Type::String)],
        Type::String,
    ));

    // File::infos(path:string) → map<string, mixed>
    methods.insert("infos".into(), m(
        vec![("path", Type::String)],
        Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
    ));

    // ── Opérations ────────────────────────────────────────────────────────────

    // File::remove(path:string) → void
    methods.insert("remove".into(), m(
        vec![("path", Type::String)],
        Type::Void,
    ));

    // File::copy(src:string, dst:string) → void
    methods.insert("copy".into(), m(
        vec![("src", Type::String), ("dst", Type::String)],
        Type::Void,
    ));

    // File::move(src:string, dst:string) → void
    methods.insert("move".into(), m(
        vec![("src", Type::String), ("dst", Type::String)],
        Type::Void,
    ));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
