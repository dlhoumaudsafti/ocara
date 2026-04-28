// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTMLComponent — classe builtin d'instance
//
// Méthodes d'instance :
//   c.register(handler: Function<string>) → void
//
// Usage :
//   var btn:HTMLComponent = use HTMLComponent("button")
//   btn.register(nameless(attrs:map<string,mixed>): string { ... })
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

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // c.register(handler: Function<string>) → void
    methods.insert("register".into(), instance(
        vec![("handler", Type::Function(Box::new(Type::String)))],
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
