// ─────────────────────────────────────────────────────────────────────────────
// ocara.Regex — classe builtin statique
//
// Méthodes statiques :
//   Regex::test(pattern, str)                → bool      teste si str correspond au pattern
//   Regex::find(pattern, str)                → string    première correspondance (vide si aucune)
//   Regex::findAll(pattern, str)            → string[]  toutes les correspondances
//   Regex::replace(pattern, str, repl)       → string    remplace la première occurrence
//   Regex::replaceAll(pattern, str, repl)   → string    remplace toutes les occurrences
//   Regex::split(pattern, str)               → string[]  découpe selon le pattern
//   Regex::count(pattern, str)               → int       nombre de correspondances
//   Regex::extract(pattern, str, group)      → string    capture d'un groupe (1-indexé)
//
// Convention runtime : Regex_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
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

    // Regex::test(pattern, str) → bool
    methods.insert("test".into(), m(
        vec![("pattern", Type::String), ("s", Type::String)],
        Type::Bool,
    ));

    // Regex::find(pattern, str) → string  (vide si aucune correspondance)
    methods.insert("find".into(), m(
        vec![("pattern", Type::String), ("s", Type::String)],
        Type::String,
    ));

    // Regex::findAll(pattern, str) → string[]
    methods.insert("findAll".into(), m(
        vec![("pattern", Type::String), ("s", Type::String)],
        Type::Array(Box::new(Type::String)),
    ));

    // Regex::replace(pattern, str, repl) → string  (première occurrence)
    methods.insert("replace".into(), m(
        vec![("pattern", Type::String), ("s", Type::String), ("repl", Type::String)],
        Type::String,
    ));

    // Regex::replaceAll(pattern, str, repl) → string  (toutes les occurrences)
    methods.insert("replaceAll".into(), m(
        vec![("pattern", Type::String), ("s", Type::String), ("repl", Type::String)],
        Type::String,
    ));

    // Regex::split(pattern, str) → string[]
    methods.insert("split".into(), m(
        vec![("pattern", Type::String), ("s", Type::String)],
        Type::Array(Box::new(Type::String)),
    ));

    // Regex::count(pattern, str) → int
    methods.insert("count".into(), m(
        vec![("pattern", Type::String), ("s", Type::String)],
        Type::Int,
    ));

    // Regex::extract(pattern, str, group) → string  (groupe de capture, 1-indexé)
    methods.insert("extract".into(), m(
        vec![("pattern", Type::String), ("s", Type::String), ("group", Type::Int)],
        Type::String,
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
