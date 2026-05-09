// ─────────────────────────────────────────────────────────────────────────────
// ocara.DotEnv — classe builtin pour charger des variables d'environnement
//
// Méthodes statiques :
//   DotEnv::load(env:string) → void
//       Charge un fichier .env dans l'environnement
//       Si env est vide/null : charge .env
//       Si env = "prod" : charge .env.prod
//       etc.
//
//   DotEnv::get(key:string) → string|null
//       Récupère une variable d'environnement
//       Retourne null si la clé n'existe pas
//
// Convention runtime : DotEnv_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

/// Helper pour méthode statique
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

/// Helper pour méthode statique avec paramètres optionnels
fn static_m_opt(params: Vec<(&str, Type)>, ret_ty: Type, required: usize) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: required,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // ── Méthodes statiques ────────────────────────────────────────────────────
    
    // DotEnv::load(env) → void
    // Charge un fichier .env (ou .env.{env} si env est fourni)
    // env est optionnel (0 paramètre requis)
    methods.insert("load".into(), static_m_opt(
        vec![("env", Type::String)],
        Type::Void,
        0,  // 0 paramètres obligatoires
    ));

    // DotEnv::get(key) → string|null
    // Récupère une variable d'environnement
    methods.insert("get".into(), static_m(
        vec![("key", Type::String)],
        Type::Union(vec![Type::String, Type::Null]),
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
