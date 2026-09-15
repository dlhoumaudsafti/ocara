// ─────────────────────────────────────────────────────────────────────────────
// ocara.Mutex — classe builtin d'instance
//
// Méthodes d'instance (is_static: false) :
//   m.lock()         → void   verrouille le mutex (bloquant)
//   m.unlock()       → void   déverrouille le mutex
//   m.tryLock()     → bool   tente de verrouiller (non-bloquant)
//   m.withLock(f:Function<void>) → void   lock + f() + unlock garanti (même si f() raise)
//   m.destroy()      → void   libère le mutex (usage après = UB, comme SQLite)
//
// Convention runtime : Mutex_<method>
// Usage :
//   var m:Mutex = use Mutex()
//   m.lock()
//   // section critique
//   m.unlock()
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn instance(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: false,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
        message_emit_in_loop: false,
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

    // m.tryLock() → bool
    methods.insert("tryLock".into(), instance(
        vec![],
        Type::Bool,
    ));

    // m.withLock(f:Function<void>) → void — lock, exécute f(), unlock garanti
    // (y compris si f() raise, contrairement à lock()/unlock() manuels — voir
    // Mutex_withLock / docs/roadmap.d/memoire-deadlocks-raise.md)
    methods.insert("withLock".into(), instance(
        vec![("f", Type::Function { ret_ty: Box::new(Type::Void), param_tys: vec![] })],
        Type::Void,
    ));

    // m.destroy() → void — libère le mutex pthread + son wrapper (voir
    // Mutex_destroy) ; sans ça, chaque `use Mutex()` fuyait indéfiniment
    methods.insert("destroy".into(), instance(
        vec![],
        Type::Void,
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
