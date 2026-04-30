// ─────────────────────────────────────────────────────────────────────────────
// ocara.Convert — classe builtin statique
//
// Conversions entre types primitifs et structures de données.
//
// string → *
//   Convert::strToInt(s)          → int
//   Convert::strToFloat(s)        → float
//   Convert::strToBool(s)         → bool
//   Convert::strToArray(s, sep)   → string[]
//   Convert::strToMap(s, sep, kv) → map<string,string>
//
// int → *
//   Convert::intToStr(n)          → string
//   Convert::intToFloat(n)        → float
//   Convert::intToBool(n)         → bool    (0 → false, sinon true)
//
// float → *
//   Convert::floatToStr(f)        → string
//   Convert::floatToInt(f)        → int     (troncature)
//   Convert::floatToBool(f)       → bool    (0.0 → false, sinon true)
//
// bool → *
//   Convert::boolToStr(b)         → string  ("true" / "false")
//   Convert::boolToInt(b)         → int     (true → 1, false → 0)
//   Convert::boolToFloat(b)       → float   (true → 1.0, false → 0.0)
//
// array → *
//   Convert::arrayToStr(arr, sep) → string  (join avec sep)
//   Convert::arrayToMap(arr, kv)  → map<string,string>
//
// map → *
//   Convert::mapToStr(m, sep, kv)         → string
//   Convert::mapKeysToArray(m)            → string[]
//   Convert::mapValuesToArray(m)          → mixed[]
//
// Convention runtime : Convert_<method>
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

    let str_arr = Type::Array(Box::new(Type::String));
    let any_arr = Type::Array(Box::new(Type::Mixed));
    let str_map = Type::Map(Box::new(Type::String), Box::new(Type::String));

    // ── string → * ────────────────────────────────────────────────────────
    methods.insert("strToInt".into(),   m(vec![("s", Type::String)],                                        Type::Int));
    methods.insert("strToFloat".into(), m(vec![("s", Type::String)],                                        Type::Float));
    methods.insert("strToBool".into(),  m(vec![("s", Type::String)],                                        Type::Bool));
    methods.insert("strToArray".into(), m(vec![("s", Type::String), ("sep", Type::String)],                 str_arr.clone()));
    methods.insert("strToMap".into(),   m(vec![("s", Type::String), ("sep", Type::String), ("kv", Type::String)], str_map.clone()));

    // ── int → * ───────────────────────────────────────────────────────────
    methods.insert("intToStr".into(),   m(vec![("n", Type::Int)],   Type::String));
    methods.insert("intToFloat".into(), m(vec![("n", Type::Int)],   Type::Float));
    methods.insert("intToBool".into(),  m(vec![("n", Type::Int)],   Type::Bool));

    // ── float → * ─────────────────────────────────────────────────────────
    methods.insert("floatToStr".into(),  m(vec![("f", Type::Float)], Type::String));
    methods.insert("floatToInt".into(),  m(vec![("f", Type::Float)], Type::Int));
    methods.insert("floatToBool".into(), m(vec![("f", Type::Float)], Type::Bool));

    // ── bool → * ──────────────────────────────────────────────────────────
    methods.insert("boolToStr".into(),   m(vec![("b", Type::Bool)],  Type::String));
    methods.insert("boolToInt".into(),   m(vec![("b", Type::Bool)],  Type::Int));
    methods.insert("boolToFloat".into(), m(vec![("b", Type::Bool)],  Type::Float));

    // ── array → * ─────────────────────────────────────────────────────────
    methods.insert("arrayToStr".into(), m(vec![("arr", any_arr.clone()), ("sep", Type::String)], Type::String));
    methods.insert("arrayToMap".into(), m(vec![("arr", any_arr.clone()), ("kv", Type::String)],  str_map.clone()));

    // ── map → * ───────────────────────────────────────────────────────────
    methods.insert("mapToStr".into(),          m(vec![("m", str_map.clone()), ("sep", Type::String), ("kv", Type::String)], Type::String));
    methods.insert("mapKeysToArray".into(),   m(vec![("m", str_map.clone())], str_arr));
    methods.insert("mapValuesToArray".into(), m(vec![("m", str_map)],         any_arr));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
