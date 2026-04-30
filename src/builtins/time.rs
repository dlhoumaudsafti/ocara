// ─────────────────────────────────────────────────────────────────────────────
// ocara.Time — classe builtin statique
//
// Méthodes statiques :
//   Time::now()                 → string  heure actuelle (HH:MM:SS)
//   Time::fromTimestamp(ts)    → string  extrait l'heure d'un timestamp (HH:MM:SS)
//   Time::hour(time)            → int     extrait l'heure (0-23)
//   Time::minute(time)          → int     extrait les minutes (0-59)
//   Time::second(time)          → int     extrait les secondes (0-59)
//   Time::fromSeconds(seconds) → string  convertit secondes → HH:MM:SS
//   Time::toSeconds(time)      → int     convertit HH:MM:SS → secondes
//   Time::addSeconds(time, s)  → string  ajoute N secondes
//   Time::diffSeconds(t1, t2)  → int     différence en secondes
//
// Convention runtime : Time_<method>
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

    methods.insert("now".into(),            m(vec![],                                                   Type::String));
    methods.insert("fromTimestamp".into(), m(vec![("ts", Type::Int)],                                  Type::String));
    methods.insert("hour".into(),           m(vec![("time", Type::String)],                             Type::Int));
    methods.insert("minute".into(),         m(vec![("time", Type::String)],                             Type::Int));
    methods.insert("second".into(),         m(vec![("time", Type::String)],                             Type::Int));
    methods.insert("fromSeconds".into(),   m(vec![("seconds", Type::Int)],                             Type::String));
    methods.insert("toSeconds".into(),     m(vec![("time", Type::String)],                             Type::Int));
    methods.insert("addSeconds".into(),    m(vec![("time", Type::String), ("s", Type::Int)],           Type::String));
    methods.insert("diffSeconds".into(),   m(vec![("t1", Type::String), ("t2", Type::String)],         Type::Int));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
