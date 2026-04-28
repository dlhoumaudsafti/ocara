// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTML — classe statique de rendu HTML
//
// Méthodes statiques :
//   HTML::render(template: string)  → string
//   HTML::escape(s: string)         → string
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn static_m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
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

    // HTML::render(template: string) → string
    methods.insert("render".into(), static_m(
        vec![("template", Type::String)],
        Type::String,
    ));

    // HTML::render_cached(template: string, cache_key: string) → string
    methods.insert("render_cached".into(), static_m(
        vec![("template", Type::String), ("cache_key", Type::String)],
        Type::String,
    ));

    // HTML::cache_delete(cache_key: string) → void
    methods.insert("cache_delete".into(), static_m(
        vec![("cache_key", Type::String)],
        Type::Void,
    ));

    // HTML::cache_clear() → void
    methods.insert("cache_clear".into(), static_m(
        vec![],
        Type::Void,
    ));

    // HTML::escape(s: string) → string
    methods.insert("escape".into(), static_m(
        vec![("s", Type::String)],
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
