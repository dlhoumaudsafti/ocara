// ─────────────────────────────────────────────────────────────────────────────
// ocara.YAML — classe builtin statique pour manipuler du YAML
//
// Méthodes statiques :
//   YAML::encode(data)  → string   encode map ou array en YAML
//   YAML::decode(yaml)  → mixed    décode string YAML en map ou array
//   YAML::parse(yaml)   → mixed    alias de decode
//
// Convention runtime : YAML_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
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
        required_params_count: len,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // YAML::encode(data) → string
    methods.insert("encode".into(), static_m(
        vec![("data", Type::Mixed)],
        Type::String,
    ));

    // YAML::decode(yaml) → mixed
    methods.insert("decode".into(), static_m(
        vec![("yaml", Type::String)],
        Type::Mixed,
    ));

    // YAML::parse(yaml) → mixed (alias de decode)
    methods.insert("parse".into(), static_m(
        vec![("yaml", Type::String)],
        Type::Mixed,
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
