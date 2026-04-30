// ─────────────────────────────────────────────────────────────────────────────
// ocara.String — classe builtin statique
//
// Convention runtime : String_<method>
//   ex. String_len, String_upper, String_lower, ...
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,  // Méthodes statiques pour String::trim(s)
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // Méthodes statiques : String::len(s) → int
    methods.insert("len".into(),        m(vec![("s", Type::String)], Type::Int));
    // String::upper(s) → string
    methods.insert("upper".into(),      m(vec![("s", Type::String)], Type::String));
    // String::lower(s) → string
    methods.insert("lower".into(),      m(vec![("s", Type::String)], Type::String));
    // String::capitalize(s) → string
    methods.insert("capitalize".into(), m(vec![("s", Type::String)], Type::String));
    // String::trim(s) → string
    methods.insert("trim".into(),       m(vec![("s", Type::String)], Type::String));
    // String::replace(s, from, to) → string
    methods.insert("replace".into(),    m(
        vec![("s", Type::String), ("from", Type::String), ("to", Type::String)],
        Type::String,
    ));
    // String::split(s, sep) → string[]
    methods.insert("split".into(),      m(
        vec![("s", Type::String), ("sep", Type::String)],
        Type::Array(Box::new(Type::String)),
    ));
    // String::explode(s, sep) → string[]  (alias de split)
    methods.insert("explode".into(),    m(
        vec![("s", Type::String), ("sep", Type::String)],
        Type::Array(Box::new(Type::String)),
    ));
    // String::between(s, start, end) → string
    methods.insert("between".into(),    m(
        vec![("s", Type::String), ("start", Type::String), ("end", Type::String)],
        Type::String,
    ));
    // String::empty(s) → bool
    methods.insert("empty".into(),      m(vec![("s", Type::String)], Type::Bool));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}

