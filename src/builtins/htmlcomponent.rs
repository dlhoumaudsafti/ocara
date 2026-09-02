// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTMLComponent — classe builtin d'instance
//
// Méthodes d'instance :
//   c.tag(name: string) → void
//   c.register(handler: Function<string>) → void
//
// Méthodes statiques :
//   HTMLComponent::unregister(name: string) → void
//
// Usage :
//   var btn:HTMLComponent = use HTMLComponent("button")
//   btn.tag("my-button")
//   btn.register(nameless(attrs:map<string,mixed>): string { ... })
//
//   // Ou lors d'un extends :
//   class Card extends HTMLComponent {
//     init() {
//       self.tag("card")
//       self.register(...)
//     }
//   }
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
        required_params_count: len,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // c.tag(name: string) → void
    methods.insert("tag".into(), instance(
        vec![("name", Type::String)],
        Type::Void,
    ));

    // c.register(handler: Function<string>) → void
    methods.insert("register".into(), instance(
        vec![("handler", Type::Function { ret_ty: Box::new(Type::String), param_tys: vec![] })],
        Type::Void,
    ));

    // HTMLComponent::unregister(name: string) → void — retire un composant du
    // registre global (voir HTML_cacheDelete pour le même patron sur un autre
    // registre du même fichier runtime). Usage réel = un petit nombre fixe de
    // composants enregistrés une fois au démarrage (fuite cosmétique sans ça),
    // ajouté en future-proofing pour un enregistrement dynamique éventuel.
    methods.insert("unregister".into(), static_m(
        vec![("name", Type::String)],
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
