// ─────────────────────────────────────────────────────────────────────────────
// ocara.SQLite — classe builtin pour base de données SQLite
//
// Méthodes statiques :
//   SQLite::open(path:string) → SQLite         // Ouvre/crée une base de données
//   SQLite::withOpen(path:string, f:Function<void(SQLite)>) → void
//     // open + f(db) + close garanti, y compris si f() raise (voir
//     // docs/roadmap.d/exceptions-setjmp-longjmp-dette.md)
//
// Méthodes d'instance — one-shot, binding nominatif `:nom` optionnel :
//   db.execute(query:string, placeholder:map<string,mixed>|null = null, close:bool = false) → void
//   db.query(query:string, placeholder:map<string,mixed>|null = null) → map<string, mixed>[]
//   db.queryOne(query:string, placeholder:map<string,mixed>|null = null) → map<string, mixed>
//
// Méthodes d'instance — stepped/transactionnel (voir
// docs/roadmap.d/stdlib-sqlite-requetes-parametrees-transactions.md et
// docs/builtins/SQLite.md pour le flux de référence) :
//   db.prepare(query:string) → void
//   db.bind(placeholders:map<string, mixed>) → void
//   db.commit(close:bool = false) → mixed     // array<map<string,mixed>> si SELECT, sinon int
//   db.rollback(close:bool = false) → void
//
//   db.lastInsertId() → int                   // Retourne l'ID de la dernière insertion
//   db.affectedRows() → int                   // Retourne le nombre de lignes affectées par la dernière requête
//   db.close() → void                         // Ferme la connexion
//
// Convention runtime : SQLite_<method> — les méthodes à paramètres optionnels
// ont des variantes `_N` (N = nombre d'arguments réels) pour les arités
// inférieures au maximum, voir `runtime/src/sqlite.rs` et
// `src/lower/expr.d/lower.rs` (dispatch par arité des builtins surchargés).
//
// Usage :
//   const db:SQLite = SQLite::open("data.db")
//   db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
//   db.execute("INSERT INTO users (name) VALUES (:name)", {"name": "Alice"})
//   const rows:map<string, mixed>[] = db.query("SELECT * FROM users")
//   db.close()
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

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

/// Méthode d'instance avec paramètres optionnels (arité variable, voir
/// `dotenv::static_m_opt` pour le même patron côté méthode statique).
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

    // SQLite::open(path:string) → SQLite
    methods.insert("open".into(), static_m(
        vec![("path", Type::String)],
        Type::Named("SQLite".to_string()),
    ));

    // SQLite::withOpen(path:string, f:Function<void(SQLite)>) → void — open,
    // exécute f(db), close garanti (y compris si f() raise, contrairement à
    // open()/close() manuels — voir SQLite_withOpen /
    // docs/roadmap.d/exceptions-setjmp-longjmp-dette.md)
    methods.insert("withOpen".into(), static_m(
        vec![
            ("path", Type::String),
            ("f", Type::Function {
                ret_ty: Box::new(Type::Void),
                param_tys: vec![Type::Named("SQLite".to_string())],
            }),
        ],
        Type::Void,
    ));

    // ── Méthodes d'instance ───────────────────────────────────────────────────

    // db.execute(query:string, placeholder:map<string,mixed>|null = null, close:bool = false) → void
    methods.insert("execute".into(), instance_opt(
        vec![
            ("query", Type::String),
            ("placeholder", placeholder_type()),
            ("close", Type::Bool),
        ],
        Type::Void,
        1,
    ));

    // db.query(query:string, placeholder:map<string,mixed>|null = null) → map<string, mixed>[]
    methods.insert("query".into(), instance_opt(
        vec![
            ("query", Type::String),
            ("placeholder", placeholder_type()),
        ],
        Type::Array(Box::new(Type::Map(Box::new(Type::String), Box::new(Type::Mixed)))),
        1,
    ));

    // db.queryOne(query:string, placeholder:map<string,mixed>|null = null) → map<string, mixed>
    methods.insert("queryOne".into(), instance_opt(
        vec![
            ("query", Type::String),
            ("placeholder", placeholder_type()),
        ],
        Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
        1,
    ));

    // db.prepare(query:string) → void — valide + mémorise la requête pour bind()/commit()
    methods.insert("prepare".into(), instance(
        vec![("query", Type::String)],
        Type::Void,
    ));

    // db.bind(placeholders:map<string, mixed>) → void — placeholders nominatifs `:nom`
    methods.insert("bind".into(), instance(
        vec![("placeholders", Type::Map(Box::new(Type::String), Box::new(Type::Mixed)))],
        Type::Void,
    ));

    // db.commit(close:bool = false) → mixed — exécute+commit la requête préparée ;
    // array<map<string,mixed>> si SELECT, sinon int (lignes affectées)
    methods.insert("commit".into(), instance_opt(
        vec![("close", Type::Bool)],
        Type::Mixed,
        0,
    ));

    // db.rollback(close:bool = false) → void — annule la transaction en cours
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
