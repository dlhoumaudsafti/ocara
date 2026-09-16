// ─────────────────────────────────────────────────────────────────────────────
// ocara.SQLite — classe builtin pour base de données SQLite
//
// Méthodes statiques :
//   SQLite::open(path:string) → SQLite         // Ouvre/crée une base de données
//   SQLite::withOpen(path:string, f:Function<void(SQLite)>) → void
//     // open + f(db) + close garanti, y compris si f() raise (voir
//     // docs/roadmap.d/exceptions-setjmp-longjmp-dette.md)
//
// Méthodes d'instance :
//   db.execute(query:string) → void           // Exécute une requête SQL (INSERT, UPDATE, DELETE, CREATE, etc.)
//   db.query(query:string) → map<string, mixed>[]  // Exécute un SELECT et retourne les résultats
//   db.queryOne(query:string) → map<string, mixed> // Exécute un SELECT et retourne une seule ligne (ou map vide)
//   db.lastInsertId() → int                   // Retourne l'ID de la dernière insertion
//   db.affectedRows() → int                   // Retourne le nombre de lignes affectées par la dernière requête
//   db.close() → void                         // Ferme la connexion
//
// Convention runtime : SQLite_<method>
//
// Usage :
//   const db:SQLite = SQLite::open("data.db")
//   db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
//   db.execute("INSERT INTO users (name) VALUES ('Alice')")
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

    // db.execute(query:string) → void
    methods.insert("execute".into(), instance(
        vec![("query", Type::String)],
        Type::Void,
    ));

    // db.query(query:string) → map<string, mixed>[]
    methods.insert("query".into(), instance(
        vec![("query", Type::String)],
        Type::Array(Box::new(Type::Map(Box::new(Type::String), Box::new(Type::Mixed)))),
    ));

    // db.queryOne(query:string) → map<string, mixed>
    methods.insert("queryOne".into(), instance(
        vec![("query", Type::String)],
        Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
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
