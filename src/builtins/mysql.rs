// ─────────────────────────────────────────────────────────────────────────────
// ocara.MySQL / ocara.MariaDB — classe builtin pour base de données MySQL/MariaDB
//
// Méthodes statiques :
//   MySQL::connect(host:string, user:string, password:string, database:string) → MySQL
//   MySQL::withConnect(host, user, password, database, f:Function<void(MySQL)>) → void
//     // connect + f(db) + close garanti, y compris si f() raise (voir
//     // docs/roadmap.d/exceptions-setjmp-longjmp-dette.md)
//
// Méthodes d'instance — one-shot, binding nominatif `:nom` optionnel :
//   db.execute(query:string, placeholder:map<string,mixed>|null = null, close:bool = false) → int
//   db.query(query:string, placeholder:map<string,mixed>|null = null) → array<map<string, mixed>>
//   db.queryOne(query:string, placeholder:map<string,mixed>|null = null) → map<string, mixed>|null
//
// Méthodes d'instance — stepped/transactionnel (voir
// docs/roadmap.d/stdlib-mysql-requetes-parametrees-transactions.md) :
//   db.prepare(query:string) → void
//   db.bind(placeholders:map<string, mixed>) → void
//   db.commit(close:bool = false) → mixed     // array<map<string,mixed>> si SELECT, sinon int
//   db.rollback(close:bool = false) → void
//
//   db.lastInsertId() → int
//   db.affectedRows() → int
//   db.close() → void
//
// Convention runtime : MySQL_<method> — les méthodes à paramètres optionnels
// ont des variantes `_N` (N = nombre d'arguments réels), voir
// `runtime/src/mysql.rs` et `src/lower/expr.d/lower.rs`.
// MariaDB est un alias de MySQL (mêmes méthodes, symboles runtime distincts).
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

/// Méthode d'instance avec paramètres optionnels (arité variable), voir
/// `sqlite::instance_opt`/`dotenv::static_m_opt` pour le même patron.
fn instance_opt(params: Vec<(&str, Type)>, ret_ty: Type, required: usize) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: false,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: required,
        message_emit_in_loop: false,
    }
}

/// `map<string, mixed>|null` — type du paramètre `placeholder` optionnel.
fn placeholder_type() -> Type {
    Type::Union(vec![
        Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
        Type::Null,
    ])
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

    // db.execute(query, placeholder:map<string,mixed>|null = null, close:bool = false) → int
    // (retourne le nombre de lignes affectées)
    methods.insert("execute".into(), instance_opt(
        vec![
            ("query", Type::String),
            ("placeholder", placeholder_type()),
            ("close", Type::Bool),
        ],
        Type::Int,
        1,
    ));

    // db.query(query, placeholder:map<string,mixed>|null = null) → array<map<string, mixed>>
    methods.insert("query".into(), instance_opt(
        vec![
            ("query", Type::String),
            ("placeholder", placeholder_type()),
        ],
        Type::Array(Box::new(Type::Map(
            Box::new(Type::String),
            Box::new(Type::Mixed),
        ))),
        1,
    ));

    // db.queryOne(query, placeholder:map<string,mixed>|null = null) → map<string, mixed>|null
    methods.insert("queryOne".into(), instance_opt(
        vec![
            ("query", Type::String),
            ("placeholder", placeholder_type()),
        ],
        Type::Union(vec![
            Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
            Type::Null,
        ]),
        1,
    ));

    // db.prepare(query:string) → void
    methods.insert("prepare".into(), instance(
        vec![("query", Type::String)],
        Type::Void,
    ));

    // db.bind(placeholders:map<string, mixed>) → void
    methods.insert("bind".into(), instance(
        vec![("placeholders", Type::Map(Box::new(Type::String), Box::new(Type::Mixed)))],
        Type::Void,
    ));

    // db.commit(close:bool = false) → mixed
    methods.insert("commit".into(), instance_opt(
        vec![("close", Type::Bool)],
        Type::Mixed,
        0,
    ));

    // db.rollback(close:bool = false) → void
    methods.insert("rollback".into(), instance_opt(
        vec![("close", Type::Bool)],
        Type::Void,
        0,
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
