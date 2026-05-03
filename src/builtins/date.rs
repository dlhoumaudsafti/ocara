// ─────────────────────────────────────────────────────────────────────────────
// ocara.Date — classe builtin statique
//
// Méthodes statiques :
//   Date::today()                    → string  date actuelle (YYYY-MM-DD)
//   Date::fromTimestamp(ts)         → string  convertit timestamp → YYYY-MM-DD
//   Date::year(date)                 → int     extrait l'année
//   Date::month(date)                → int     extrait le mois (1-12)
//   Date::day(date)                  → int     extrait le jour (1-31)
//   Date::dayOfWeek(date)          → int     jour de la semaine (0=lundi, 6=dimanche)
//   Date::isLeapYear(year)         → bool    année bissextile ?
//   Date::daysInMonth(year, month) → int     nombre de jours dans le mois
//   Date::addDays(date, days)       → string  ajoute N jours
//   Date::diffDays(date1, date2)    → int     différence en jours
//
// Convention runtime : Date_<method>
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

    methods.insert("today".into(),          m(vec![],                                                           Type::String));
    methods.insert("fromTimestamp".into(), m(vec![("ts", Type::Int)],                                          Type::String));
    methods.insert("year".into(),           m(vec![("date", Type::String)],                                     Type::Int));
    methods.insert("month".into(),          m(vec![("date", Type::String)],                                     Type::Int));
    methods.insert("day".into(),            m(vec![("date", Type::String)],                                     Type::Int));
    methods.insert("dayOfWeek".into(),    m(vec![("date", Type::String)],                                     Type::Int));
    methods.insert("isLeapYear".into(),   m(vec![("year", Type::Int)],                                        Type::Bool));
    methods.insert("daysInMonth".into(),  m(vec![("year", Type::Int), ("month", Type::Int)],                  Type::Int));
    methods.insert("addDays".into(),       m(vec![("date", Type::String), ("days", Type::Int)],                Type::String));
    methods.insert("diffDays".into(),      m(vec![("date1", Type::String), ("date2", Type::String)],           Type::Int));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
