// ─────────────────────────────────────────────────────────────────────────────
// ocara.Math — classe builtin statique
//
// Constantes de classe :
//   Math::PI       → float  (3.14159265358979)
//   Math::E        → float  (2.71828182845904)
//   Math::TAU      → float  (6.28318530717958)
//   Math::INF      → float  (infini positif)
//
// Méthodes statiques :
//   Math::abs(n)              → int    valeur absolue
//   Math::min(a, b)           → int    minimum
//   Math::max(a, b)           → int    maximum
//   Math::pow(base, exp)      → int    puissance entière
//   Math::clamp(n, min, max)  → int    borne entre min et max
//   Math::sqrt(n)             → float  racine carrée
//   Math::floor(n)            → int    arrondi inférieur
//   Math::ceil(n)             → int    arrondi supérieur
//   Math::round(n)            → int    arrondi au plus proche
//
// Convention runtime : Math_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::{Type, Visibility};
use crate::sema::symbols::{ClassInfo, FuncSig};

fn m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
        is_async:  false,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // Méthodes entières
    methods.insert("abs".into(),   m(vec![("n", Type::Int)],                                       Type::Int));
    methods.insert("min".into(),   m(vec![("a", Type::Int), ("b", Type::Int)],                     Type::Int));
    methods.insert("max".into(),   m(vec![("a", Type::Int), ("b", Type::Int)],                     Type::Int));
    methods.insert("pow".into(),   m(vec![("base", Type::Int), ("exp", Type::Int)],                Type::Int));
    methods.insert("clamp".into(), m(vec![("n", Type::Int), ("lo", Type::Int), ("hi", Type::Int)], Type::Int));

    // Méthodes flottantes
    methods.insert("sqrt".into(),  m(vec![("n", Type::Float)], Type::Float));
    methods.insert("floor".into(), m(vec![("n", Type::Float)], Type::Int));
    methods.insert("ceil".into(),  m(vec![("n", Type::Float)], Type::Int));
    methods.insert("round".into(), m(vec![("n", Type::Float)], Type::Int));

    // Constantes de classe
    let mut class_consts: HashMap<String, (Type, Visibility)> = HashMap::new();
    class_consts.insert("PI".into(),  (Type::Float, Visibility::Public));
    class_consts.insert("E".into(),   (Type::Float, Visibility::Public));
    class_consts.insert("TAU".into(), (Type::Float, Visibility::Public));
    class_consts.insert("INF".into(), (Type::Float, Visibility::Public));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts,
        is_opaque:    false,
    }
}

