// ─────────────────────────────────────────────────────────────────────────────
// ocara.Thread — classe builtin d'instance
//
// Méthodes d'instance (is_static: false) :
//   t.run(f:Function)  → void   lance le thread avec une closure
//   t.join()           → void   attend la fin
//   t.detach()         → void   détache (fire-and-forget)
//   t.id()             → int    ID du thread
//
// Méthodes statiques (is_static: true) :
//   Thread::sleep(ms:int)   → void   pause ms millisecondes
//   Thread::current_id()    → int    ID du thread courant
//
// Convention runtime : Thread_<method>
// Usage :
//   var t:Thread = use Thread()
//   t.run(nameless(): void { ... })
//   t.join()
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
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
    }
}

fn static_m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
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

    // ── Méthodes d'instance ───────────────────────────────────────────────────

    // t.run(f:Function<void>) → void
    methods.insert("run".into(), instance(
        vec![("f", Type::Function(Box::new(Type::Void)))],
        Type::Void,
    ));

    // t.join() → void
    methods.insert("join".into(), instance(
        vec![],
        Type::Void,
    ));

    // t.detach() → void
    methods.insert("detach".into(), instance(
        vec![],
        Type::Void,
    ));

    // t.id() → int
    methods.insert("id".into(), instance(
        vec![],
        Type::Int,
    ));

    // ── Méthodes statiques ────────────────────────────────────────────────────

    // Thread::sleep(ms:int) → void
    methods.insert("sleep".into(), static_m(
        vec![("ms", Type::Int)],
        Type::Void,
    ));

    // Thread::current_id() → int
    methods.insert("current_id".into(), static_m(
        vec![],
        Type::Int,
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
