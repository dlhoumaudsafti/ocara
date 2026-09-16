// ─────────────────────────────────────────────────────────────────────────────
// ocara.MySQL / ocara.MariaDB — classe builtin pour base de données MySQL/MariaDB
//
// Méthodes statiques :
//   MySQL::connect(host:string, user:string, password:string, database:string) → MySQL
//   MySQL::withConnect(host, user, password, database, f:Function<void(MySQL)>) → void
//     // connect + f(db) + close garanti, y compris si f() raise (voir
//     // docs/roadmap.d/exceptions-setjmp-longjmp-dette.md)
//
// Méthodes d'instance :
//   db.execute(query:string) → int
//   db.query(query:string) → array<map<string, mixed>>
//   db.queryOne(query:string) → map<string, mixed>|null
//   db.lastInsertId() → int
//   db.affectedRows() → int
//   db.close() → void
//
// Convention runtime : MySQL_<method>
// MariaDB est un alias de MySQL
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
        message_emit_in_loop: false,
    }
}

/// Helper pour méthode d'instance
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

    // ── Méthode statique ──────────────────────────────────────────────────────
    
    // MySQL::connect(host, user, password, database) → MySQL
    methods.insert("connect".into(), static_m(
        vec![
            ("host", Type::String),
            ("user", Type::String),
            ("password", Type::String),
            ("database", Type::String),
        ],
        Type::Named("MySQL".to_string()),
    ));

    // MySQL::withConnect(host, user, password, database, f:Function<void(MySQL)>) → void
    // — connect, exécute f(db), close garanti (y compris si f() raise,
    // contrairement à connect()/close() manuels — voir MySQL_withConnect /
    // docs/roadmap.d/exceptions-setjmp-longjmp-dette.md)
    methods.insert("withConnect".into(), static_m(
        vec![
            ("host", Type::String),
            ("user", Type::String),
            ("password", Type::String),
            ("database", Type::String),
            ("f", Type::Function {
                ret_ty: Box::new(Type::Void),
                param_tys: vec![Type::Named("MySQL".to_string())],
            }),
        ],
        Type::Void,
    ));

    // ── Méthodes d'instance ───────────────────────────────────────────────────

    // db.execute(query) → int (retourne le nombre de lignes affectées)
    methods.insert("execute".into(), instance(
        vec![("query", Type::String)],
        Type::Int,
    ));

    // db.query(query) → array<map<string, mixed>>
    methods.insert("query".into(), instance(
        vec![("query", Type::String)],
        Type::Array(Box::new(Type::Map(
            Box::new(Type::String),
            Box::new(Type::Mixed),
        ))),
    ));

    // db.queryOne(query) → map<string, mixed>|null
    methods.insert("queryOne".into(), instance(
        vec![("query", Type::String)],
        Type::Union(vec![
            Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
            Type::Null,
        ]),
    ));

    // db.lastInsertId() → int
    methods.insert("lastInsertId".into(), instance(
        vec![],
        Type::Int,
    ));

    // db.affectedRows() → int
    methods.insert("affectedRows".into(), instance(
        vec![],
        Type::Int,
    ));

    // db.close() → void
    methods.insert("close".into(), instance(
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
