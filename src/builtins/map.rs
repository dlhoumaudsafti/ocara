// ─────────────────────────────────────────────────────────────────────────────
// ocara.Map — classe builtin statique
//
// Méthodes statiques :
//   Map::size(m)              → int     nombre de clés
//   Map::has(m, key)          → bool    vrai si la clé existe
//   Map::get(m, key)          → mixed     valeur associée (mixed si absente)
//   Map::set(m, key, val)     → void    insère ou met à jour une entrée
//   Map::remove(m, key)       → void    supprime une entrée
//   Map::keys(m)              → mixed[]   tableau de toutes les clés
//   Map::values(m)            → mixed[]   tableau de toutes les valeurs
//   Map::merge(a, b)          → map     fusion (b écrase les clés communes)
//   Map::is_empty(m)          → bool    vrai si aucune entrée
//
// Convention runtime : Map_<method>
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

    let map_ty  = Type::Map(Box::new(Type::Mixed), Box::new(Type::Mixed));
    let any_arr = Type::Array(Box::new(Type::Mixed));

    // Map::size(m) → int
    methods.insert("size".into(), m(
        vec![("m", map_ty.clone())],
        Type::Int,
    ));

    // Map::has(m, key) → bool
    methods.insert("has".into(), m(
        vec![("m", map_ty.clone()), ("key", Type::Mixed)],
        Type::Bool,
    ));

    // Map::get(m, key) → mixed
    methods.insert("get".into(), m(
        vec![("m", map_ty.clone()), ("key", Type::Mixed)],
        Type::Mixed,
    ));

    // Map::set(m, key, val) → void
    methods.insert("set".into(), m(
        vec![("m", map_ty.clone()), ("key", Type::Mixed), ("val", Type::Mixed)],
        Type::Void,
    ));

    // Map::remove(m, key) → void
    methods.insert("remove".into(), m(
        vec![("m", map_ty.clone()), ("key", Type::Mixed)],
        Type::Void,
    ));

    // Map::keys(m) → mixed[]
    methods.insert("keys".into(), m(
        vec![("m", map_ty.clone())],
        any_arr.clone(),
    ));

    // Map::values(m) → mixed[]
    methods.insert("values".into(), m(
        vec![("m", map_ty.clone())],
        any_arr,
    ));

    // Map::merge(a, b) → map  (b écrase les clés communes)
    methods.insert("merge".into(), m(
        vec![("a", map_ty.clone()), ("b", map_ty.clone())],
        map_ty.clone(),
    ));

    // Map::is_empty(m) → bool
    methods.insert("is_empty".into(), m(
        vec![("m", map_ty)],
        Type::Bool,
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
