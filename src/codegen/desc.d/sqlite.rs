use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module SQLite
pub const SQLITE_BUILTINS: &[BuiltinDesc] = &[
    // Méthode statique
    BuiltinDesc { name: "SQLite_open",          params: &[clt::I64],               returns: Some(clt::I64), module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_withOpen",      params: &[clt::I64, clt::I64],     returns: None,           module: Some("SQLite") },

    // Méthodes d'instance — one-shot, arité variable (placeholder/close optionnels)
    BuiltinDesc { name: "SQLite_execute_1",     params: &[clt::I64, clt::I64],                         returns: None,           module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_execute_2",     params: &[clt::I64, clt::I64, clt::I64],               returns: None,           module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_execute",       params: &[clt::I64, clt::I64, clt::I64, clt::I64],     returns: None,           module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_query_1",       params: &[clt::I64, clt::I64],                         returns: Some(clt::I64), module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_query",         params: &[clt::I64, clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_queryOne_1",    params: &[clt::I64, clt::I64],                         returns: Some(clt::I64), module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_queryOne",      params: &[clt::I64, clt::I64, clt::I64],               returns: Some(clt::I64), module: Some("SQLite") },

    // Méthodes d'instance — stepped/transactionnel
    BuiltinDesc { name: "SQLite_prepare",       params: &[clt::I64, clt::I64],     returns: None,           module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_bind",          params: &[clt::I64, clt::I64],     returns: None,           module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_commit_0",      params: &[clt::I64],               returns: Some(clt::I64), module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_commit",        params: &[clt::I64, clt::I64],     returns: Some(clt::I64), module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_rollback_0",    params: &[clt::I64],               returns: None,           module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_rollback",      params: &[clt::I64, clt::I64],     returns: None,           module: Some("SQLite") },

    BuiltinDesc { name: "SQLite_lastInsertId",  params: &[clt::I64],               returns: Some(clt::I64), module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_affectedRows",  params: &[clt::I64],               returns: Some(clt::I64), module: Some("SQLite") },
    BuiltinDesc { name: "SQLite_close",         params: &[clt::I64],               returns: None,           module: Some("SQLite") },
];
