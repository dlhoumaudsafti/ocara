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
    
    // Méthodes d'instance
    BuiltinDesc { 
        name: "MySQL_execute", 
        params: &[clt::I64, clt::I64],  // self_ptr, query
        returns: Some(clt::I64),        // → int (affected rows)
        module: Some("MySQL") 
    },
    BuiltinDesc { 
        name: "MySQL_query", 
        params: &[clt::I64, clt::I64],  // self_ptr, query
        returns: Some(clt::I64),        // → array<map<string, mixed>> (pointeur)
        module: Some("MySQL") 
    },
    BuiltinDesc { 
        name: "MySQL_queryOne", 
        params: &[clt::I64, clt::I64],  // self_ptr, query
        returns: Some(clt::I64),        // → map<string, mixed>|null (pointeur ou 0)
        module: Some("MySQL") 
    },
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
        name: "MariaDB_execute", 
        params: &[clt::I64, clt::I64],
        returns: Some(clt::I64),
        module: Some("MariaDB") 
    },
    BuiltinDesc { 
        name: "MariaDB_query", 
        params: &[clt::I64, clt::I64],
        returns: Some(clt::I64),
        module: Some("MariaDB") 
    },
    BuiltinDesc { 
        name: "MariaDB_queryOne", 
        params: &[clt::I64, clt::I64],
        returns: Some(clt::I64),
        module: Some("MariaDB") 
    },
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
