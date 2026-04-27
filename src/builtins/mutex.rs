// ─────────────────────────────────────────────────────────────────────────────
// ocara.Mutex — classe builtin d'instance
//
// Méthodes d'instance (is_static: false) :
//   m.lock()         → void   verrouille le mutex (bloquant)
//   m.unlock()       → void   déverrouille le mutex
//   m.try_lock()     → bool   tente de verrouiller (non-bloquant)
//
// Convention runtime : Mutex_<method>
// Usage :
//   var m:Mutex = use Mutex()
//   m.lock()
//   // section critique
//   m.unlock()
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn instance(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: false,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // ── Méthodes d'instance ───────────────────────────────────────────────────

    // m.lock() → void
    methods.insert("lock".into(), instance(
        vec![],
        Type::Void,
    ));

    // m.unlock() → void
    methods.insert("unlock".into(), instance(
        vec![],
        Type::Void,
    ));

    // m.try_lock() → bool
    methods.insert("try_lock".into(), instance(
        vec![],
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
