// ─────────────────────────────────────────────────────────────────────────────
// ocara.JSON — classe builtin statique
//
// Méthodes statiques :
//   JSON::encode(data)      → string   encode map ou array en JSON
//   JSON::decode(json)      → mixed    décode string JSON en map ou array
//   JSON::pretty(json)      → string   formatte le JSON avec indentation
//   JSON::minimize(json)    → string   minifie le JSON (supprime espaces)
//
// Les méthodes sont aussi disponibles comme méthodes d'instance :
//   - map/array peuvent appeler .encode()
//   - string peut appeler .decode(), .pretty(), .minimize()
//
// Convention runtime : JSON_<method>
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

    // JSON::encode(data) → string
    // data peut être map<K,V> ou T[]
    methods.insert("encode".into(), m(
        vec![("data", Type::Mixed)],
        Type::String,
    ));

    // JSON::decode(json) → mixed (retourne map ou array selon le JSON)
    methods.insert("decode".into(), m(
        vec![("json", Type::String)],
        Type::Mixed,
    ));

    // JSON::pretty(json) → string
    methods.insert("pretty".into(), m(
        vec![("json", Type::String)],
        Type::String,
    ));

    // JSON::minimize(json) → string
    methods.insert("minimize".into(), m(
        vec![("json", Type::String)],
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
