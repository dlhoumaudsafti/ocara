// ─────────────────────────────────────────────────────────────────────────────
// ocara.Array — classe builtin statique
//
// Méthodes statiques :
//   Array::len(arr)              → int     nombre d'éléments
//   Array::push(arr, val)        → void    ajoute val à la fin
//   Array::pop(arr)              → mixed     retire et retourne le dernier élément
//   Array::first(arr)            → mixed     premier élément
//   Array::last(arr)             → mixed     dernier élément
//   Array::contains(arr, val)    → bool    vrai si val est présent
//   Array::index_of(arr, val)    → int     index de val (-1 si absent)
//   Array::reverse(arr)          → mixed[]   tableau inversé (nouvel array)
//   Array::slice(arr, from, to)  → mixed[]   sous-tableau [from, to) (nouvel array)
//   Array::join(arr, sep)        → string  concatène les éléments avec le séparateur
//   Array::sort(arr)             → mixed[]   tableau trié (ordre naturel, nouvel array)
//
// Convention runtime : Array_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    let any_arr = Type::Array(Box::new(Type::Mixed));

    // Array::len(arr) → int
    methods.insert("len".into(), m(
        vec![("arr", any_arr.clone())],
        Type::Int,
    ));

    // Array::push(arr, val) → void
    methods.insert("push".into(), m(
        vec![("arr", any_arr.clone()), ("val", Type::Mixed)],
        Type::Void,
    ));

    // Array::pop(arr) → mixed
    methods.insert("pop".into(), m(
        vec![("arr", any_arr.clone())],
        Type::Mixed,
    ));

    // Array::first(arr) → mixed
    methods.insert("first".into(), m(
        vec![("arr", any_arr.clone())],
        Type::Mixed,
    ));

    // Array::last(arr) → mixed
    methods.insert("last".into(), m(
        vec![("arr", any_arr.clone())],
        Type::Mixed,
    ));

    // Array::contains(arr, val) → bool
    methods.insert("contains".into(), m(
        vec![("arr", any_arr.clone()), ("val", Type::Mixed)],
        Type::Bool,
    ));

    // Array::index_of(arr, val) → int  (-1 si absent)
    methods.insert("index_of".into(), m(
        vec![("arr", any_arr.clone()), ("val", Type::Mixed)],
        Type::Int,
    ));

    // Array::reverse(arr) → mixed[]
    methods.insert("reverse".into(), m(
        vec![("arr", any_arr.clone())],
        any_arr.clone(),
    ));

    // Array::slice(arr, from, to) → mixed[]
    methods.insert("slice".into(), m(
        vec![("arr", any_arr.clone()), ("from", Type::Int), ("to", Type::Int)],
        any_arr.clone(),
    ));

    // Array::join(arr, sep) → string
    methods.insert("join".into(), m(
        vec![("arr", any_arr.clone()), ("sep", Type::String)],
        Type::String,
    ));

    // Array::sort(arr) → mixed[]
    methods.insert("sort".into(), m(
        vec![("arr", any_arr.clone())],
        any_arr,
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
