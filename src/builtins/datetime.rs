// ─────────────────────────────────────────────────────────────────────────────
// ocara.DateTime — classe builtin statique
//
// Méthodes statiques :
//   DateTime::now()                → int     timestamp Unix actuel
//   DateTime::fromTimestamp(ts)   → string  convertit timestamp en ISO 8601
//   DateTime::year(ts)             → int     extrait l'année
//   DateTime::month(ts)            → int     extrait le mois (1-12)
//   DateTime::day(ts)              → int     extrait le jour (1-31)
//   DateTime::hour(ts)             → int     extrait l'heure (0-23)
//   DateTime::minute(ts)           → int     extrait les minutes (0-59)
//   DateTime::second(ts)           → int     extrait les secondes (0-59)
//   DateTime::format(ts, fmt)      → string  formate selon pattern
//   DateTime::parse(s)             → int     parse ISO 8601 → timestamp
//
// Convention runtime : DateTime_<method>
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

    methods.insert("now".into(),            m(vec![],                                               Type::Int));
    methods.insert("fromTimestamp".into(), m(vec![("ts", Type::Int)],                              Type::String));
    methods.insert("year".into(),           m(vec![("ts", Type::Int)],                              Type::Int));
    methods.insert("month".into(),          m(vec![("ts", Type::Int)],                              Type::Int));
    methods.insert("day".into(),            m(vec![("ts", Type::Int)],                              Type::Int));
    methods.insert("hour".into(),           m(vec![("ts", Type::Int)],                              Type::Int));
    methods.insert("minute".into(),         m(vec![("ts", Type::Int)],                              Type::Int));
    methods.insert("second".into(),         m(vec![("ts", Type::Int)],                              Type::Int));
    methods.insert("format".into(),         m(vec![("ts", Type::Int), ("fmt", Type::String)],       Type::String));
    methods.insert("parse".into(),          m(vec![("s", Type::String)],                            Type::Int));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
