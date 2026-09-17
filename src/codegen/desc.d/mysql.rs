use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module MySQL/MariaDB
pub const MYSQL_BUILTINS: &[BuiltinDesc] = &[
    // Méthode statique
    BuiltinDesc {
        name: "MySQL_connect",
        params: &[clt::I64, clt::I64, clt::I64, clt::I64],  // host, user, password, database
        returns: Some(clt::I64),                             // → MySQL (pointeur)
        module: Some("MySQL")
    },
    BuiltinDesc {
        name: "MySQL_withConnect",
        params: &[clt::I64, clt::I64, clt::I64, clt::I64, clt::I64],  // host, user, password, database, fat_ptr
        returns: None,
        module: Some("MySQL")
    },

    // Méthodes d'instance — one-shot, arité variable (placeholder/close optionnels)
    BuiltinDesc { name: "MySQL_execute_1", params: &[clt::I64, clt::I64],                     returns: Some(clt::I64), module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_execute_2", params: &[clt::I64, clt::I64, clt::I64],           returns: Some(clt::I64), module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_execute",   params: &[clt::I64, clt::I64, clt::I64, clt::I64], returns: Some(clt::I64), module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_query_1",   params: &[clt::I64, clt::I64],                     returns: Some(clt::I64), module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_query",     params: &[clt::I64, clt::I64, clt::I64],           returns: Some(clt::I64), module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_queryOne_1", params: &[clt::I64, clt::I64],                    returns: Some(clt::I64), module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_queryOne",  params: &[clt::I64, clt::I64, clt::I64],           returns: Some(clt::I64), module: Some("MySQL") },

    // Méthodes d'instance — stepped/transactionnel
    BuiltinDesc { name: "MySQL_prepare",    params: &[clt::I64, clt::I64], returns: None,           module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_bind",       params: &[clt::I64, clt::I64], returns: None,           module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_commit_0",   params: &[clt::I64],           returns: Some(clt::I64), module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_commit",     params: &[clt::I64, clt::I64], returns: Some(clt::I64), module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_rollback_0", params: &[clt::I64],           returns: None,           module: Some("MySQL") },
    BuiltinDesc { name: "MySQL_rollback",   params: &[clt::I64, clt::I64], returns: None,           module: Some("MySQL") },

    BuiltinDesc {
        name: "MySQL_lastInsertId",
        params: &[clt::I64],            // self_ptr
        returns: Some(clt::I64),        // → int
        module: Some("MySQL") 
    },
    BuiltinDesc { 
        name: "MySQL_affectedRows", 
        params: &[clt::I64],            // self_ptr
        returns: Some(clt::I64),        // → int
        module: Some("MySQL") 
    },
    BuiltinDesc { 
        name: "MySQL_close", 
        params: &[clt::I64],            // self_ptr
        returns: None,                  // → void
        module: Some("MySQL") 
    },

    // MariaDB alias - mêmes signatures
    BuiltinDesc {
        name: "MariaDB_connect",
        params: &[clt::I64, clt::I64, clt::I64, clt::I64],
        returns: Some(clt::I64),
        module: Some("MariaDB")
    },
    BuiltinDesc {
        name: "MariaDB_withConnect",
        params: &[clt::I64, clt::I64, clt::I64, clt::I64, clt::I64],
        returns: None,
        module: Some("MariaDB")
    },
    BuiltinDesc { name: "MariaDB_execute_1", params: &[clt::I64, clt::I64],                     returns: Some(clt::I64), module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_execute_2", params: &[clt::I64, clt::I64, clt::I64],           returns: Some(clt::I64), module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_execute",   params: &[clt::I64, clt::I64, clt::I64, clt::I64], returns: Some(clt::I64), module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_query_1",   params: &[clt::I64, clt::I64],                     returns: Some(clt::I64), module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_query",     params: &[clt::I64, clt::I64, clt::I64],           returns: Some(clt::I64), module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_queryOne_1", params: &[clt::I64, clt::I64],                    returns: Some(clt::I64), module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_queryOne",  params: &[clt::I64, clt::I64, clt::I64],           returns: Some(clt::I64), module: Some("MariaDB") },

    BuiltinDesc { name: "MariaDB_prepare",    params: &[clt::I64, clt::I64], returns: None,           module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_bind",       params: &[clt::I64, clt::I64], returns: None,           module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_commit_0",   params: &[clt::I64],           returns: Some(clt::I64), module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_commit",     params: &[clt::I64, clt::I64], returns: Some(clt::I64), module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_rollback_0", params: &[clt::I64],           returns: None,           module: Some("MariaDB") },
    BuiltinDesc { name: "MariaDB_rollback",   params: &[clt::I64, clt::I64], returns: None,           module: Some("MariaDB") },

    BuiltinDesc {
        name: "MariaDB_lastInsertId",
        params: &[clt::I64],
        returns: Some(clt::I64),
        module: Some("MariaDB") 
    },
    BuiltinDesc { 
        name: "MariaDB_affectedRows", 
        params: &[clt::I64],
        returns: Some(clt::I64),
        module: Some("MariaDB") 
    },
    BuiltinDesc { 
        name: "MariaDB_close", 
        params: &[clt::I64],
        returns: None,
        module: Some("MariaDB") 
    },
];
