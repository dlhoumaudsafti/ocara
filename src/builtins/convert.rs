// ─────────────────────────────────────────────────────────────────────────────
// ocara.Convert — classe builtin statique
//
// Conversions entre types primitifs et structures de données.
//
// string → *
//   Convert::str_to_int(s)          → int
//   Convert::str_to_float(s)        → float
//   Convert::str_to_bool(s)         → bool
//   Convert::str_to_array(s, sep)   → string[]
//   Convert::str_to_map(s, sep, kv) → map<string,string>
//
// int → *
//   Convert::int_to_str(n)          → string
//   Convert::int_to_float(n)        → float
//   Convert::int_to_bool(n)         → bool    (0 → false, sinon true)
//
// float → *
//   Convert::float_to_str(f)        → string
//   Convert::float_to_int(f)        → int     (troncature)
//   Convert::float_to_bool(f)       → bool    (0.0 → false, sinon true)
//
// bool → *
//   Convert::bool_to_str(b)         → string  ("true" / "false")
//   Convert::bool_to_int(b)         → int     (true → 1, false → 0)
//   Convert::bool_to_float(b)       → float   (true → 1.0, false → 0.0)
//
// array → *
//   Convert::array_to_str(arr, sep) → string  (join avec sep)
//   Convert::array_to_map(arr, kv)  → map<string,string>
//
// map → *
//   Convert::map_to_str(m, sep, kv)         → string
//   Convert::map_keys_to_array(m)            → string[]
//   Convert::map_values_to_array(m)          → mixed[]
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
    methods.insert("str_to_int".into(),   m(vec![("s", Type::String)],                                        Type::Int));
    methods.insert("str_to_float".into(), m(vec![("s", Type::String)],                                        Type::Float));
    methods.insert("str_to_bool".into(),  m(vec![("s", Type::String)],                                        Type::Bool));
    methods.insert("str_to_array".into(), m(vec![("s", Type::String), ("sep", Type::String)],                 str_arr.clone()));
    methods.insert("str_to_map".into(),   m(vec![("s", Type::String), ("sep", Type::String), ("kv", Type::String)], str_map.clone()));

    // ── int → * ───────────────────────────────────────────────────────────
    methods.insert("int_to_str".into(),   m(vec![("n", Type::Int)],   Type::String));
    methods.insert("int_to_float".into(), m(vec![("n", Type::Int)],   Type::Float));
    methods.insert("int_to_bool".into(),  m(vec![("n", Type::Int)],   Type::Bool));

    // ── float → * ─────────────────────────────────────────────────────────
    methods.insert("float_to_str".into(),  m(vec![("f", Type::Float)], Type::String));
    methods.insert("float_to_int".into(),  m(vec![("f", Type::Float)], Type::Int));
    methods.insert("float_to_bool".into(), m(vec![("f", Type::Float)], Type::Bool));

    // ── bool → * ──────────────────────────────────────────────────────────
    methods.insert("bool_to_str".into(),   m(vec![("b", Type::Bool)],  Type::String));
    methods.insert("bool_to_int".into(),   m(vec![("b", Type::Bool)],  Type::Int));
    methods.insert("bool_to_float".into(), m(vec![("b", Type::Bool)],  Type::Float));

    // ── array → * ─────────────────────────────────────────────────────────
    methods.insert("array_to_str".into(), m(vec![("arr", any_arr.clone()), ("sep", Type::String)], Type::String));
    methods.insert("array_to_map".into(), m(vec![("arr", any_arr.clone()), ("kv", Type::String)],  str_map.clone()));

    // ── map → * ───────────────────────────────────────────────────────────
    methods.insert("map_to_str".into(),          m(vec![("m", str_map.clone()), ("sep", Type::String), ("kv", Type::String)], Type::String));
    methods.insert("map_keys_to_array".into(),   m(vec![("m", str_map.clone())], str_arr));
    methods.insert("map_values_to_array".into(), m(vec![("m", str_map)],         any_arr));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
