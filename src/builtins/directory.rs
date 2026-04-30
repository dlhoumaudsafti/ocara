// ─────────────────────────────────────────────────────────────────────────────
// ocara.Directory — classe builtin statique pour opérations sur répertoires
//
// Méthodes statiques :
//   Directory::create(path:string|Directory) → void
//   Directory::createRecursive(path:string|Directory) → void
//   Directory::remove(path:string|Directory) → void
//   Directory::removeRecursive(path:string|Directory) → void
//   Directory::list(path:string|Directory) → string[]
//   Directory::listFiles(path:string|Directory) → string[]
//   Directory::listDirs(path:string|Directory) → string[]
//   Directory::exists(path:string|Directory) → bool
//   Directory::count(path:string|Directory) → int
//   Directory::copy(src:string|Directory, dst:string|Directory) → void
//   Directory::move(src:string|Directory, dst:string|Directory) → void
//   Directory::infos(path:string|Directory) → map<string, mixed>
//
// Convention runtime : Directory_<method>
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
        required_params_count: len,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // ── Création/Suppression ──────────────────────────────────────────────────

    // Directory::create(path:string) → void
    methods.insert("create".into(), m(
        vec![("path", Type::String)],
        Type::Void,
    ));

    // Directory::createRecursive(path:string) → void
    methods.insert("createRecursive".into(), m(
        vec![("path", Type::String)],
        Type::Void,
    ));

    // Directory::remove(path:string) → void
    methods.insert("remove".into(), m(
        vec![("path", Type::String)],
        Type::Void,
    ));

    // Directory::removeRecursive(path:string) → void
    methods.insert("removeRecursive".into(), m(
        vec![("path", Type::String)],
        Type::Void,
    ));

    // ── Listing ───────────────────────────────────────────────────────────────

    // Directory::list(path:string) → string[]
    methods.insert("list".into(), m(
        vec![("path", Type::String)],
        Type::Array(Box::new(Type::String)),
    ));

    // Directory::listFiles(path:string) → string[]
    methods.insert("listFiles".into(), m(
        vec![("path", Type::String)],
        Type::Array(Box::new(Type::String)),
    ));

    // Directory::listDirs(path:string) → string[]
    methods.insert("listDirs".into(), m(
        vec![("path", Type::String)],
        Type::Array(Box::new(Type::String)),
    ));

    // ── Métadonnées ───────────────────────────────────────────────────────────

    // Directory::exists(path:string) → bool
    methods.insert("exists".into(), m(
        vec![("path", Type::String)],
        Type::Bool,
    ));

    // Directory::count(path:string) → int
    methods.insert("count".into(), m(
        vec![("path", Type::String)],
        Type::Int,
    ));

    // Directory::infos(path:string) → map<string, mixed>
    methods.insert("infos".into(), m(
        vec![("path", Type::String)],
        Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
    ));

    // ── Opérations ────────────────────────────────────────────────────────────

    // Directory::copy(src:string, dst:string) → void
    methods.insert("copy".into(), m(
        vec![("src", Type::String), ("dst", Type::String)],
        Type::Void,
    ));

    // Directory::move(src:string, dst:string) → void
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
