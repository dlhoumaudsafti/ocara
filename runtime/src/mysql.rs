// ─────────────────────────────────────────────────────────────────────────────
// ocara.MySQL / ocara.MariaDB — Base de données MySQL/MariaDB
//
// Fonctions exportées (convention C) :
//
//   MySQL_connect(host, user, password, database) → i64  // pointeur vers OcaraMySQLDatabase
//   MySQL_execute(db_ptr, query)                   → i64  // nombre de lignes affectées
//   MySQL_query(db_ptr, query)                     → i64  // pointeur vers array de maps
//   MySQL_queryOne(db_ptr, query)                  → i64  // pointeur vers map ou 0
//   MySQL_lastInsertId(db_ptr)                     → i64
//   MySQL_affectedRows(db_ptr)                     → i64
//   MySQL_close(db_ptr)                            → void
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::Mutex;
use mysql::{Pool, OptsBuilder};
use mysql::prelude::*;
use crate::{alloc_str, ptr_to_str};

/// Structure interne représentant une connexion MySQL
pub struct OcaraMySQLDatabase {
    pool: Mutex<Pool>,
    last_insert_id: Mutex<i64>,
    affected_rows: Mutex<i64>,
}

/// MySQL::connect(host, user, password, database) → MySQL
/// Crée une connexion à une base de données MySQL/MariaDB
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_connect(
    host_ptr: i64,
    user_ptr: i64,
    password_ptr: i64,
    database_ptr: i64,
) -> i64 {
    unsafe {
        if host_ptr == 0 || user_ptr == 0 || password_ptr == 0 || database_ptr == 0 {
            eprintln!("[MySQL] Error: null parameter in connect");
            return 0;
        }
    
        let host = ptr_to_str(host_ptr);
        let user = ptr_to_str(user_ptr);
        let password = ptr_to_str(password_ptr);
        let database = ptr_to_str(database_ptr);
    
        let opts = OptsBuilder::new()
            .ip_or_hostname(Some(host))
            .user(Some(user))
            .pass(Some(password))
            .db_name(Some(database));
    
        match Pool::new(opts) {
            Ok(pool) => {
                let db = Box::new(OcaraMySQLDatabase {
                    pool: Mutex::new(pool),
                    last_insert_id: Mutex::new(0),
                    affected_rows: Mutex::new(0),
                });
                Box::into_raw(db) as i64
            }
            Err(e) => {
                eprintln!("[MySQL] Connection error: {}", e);
                0
            }
        }
    }
}

/// db.execute(query) → int
/// Exécute une requête SQL (INSERT, UPDATE, DELETE, CREATE, etc.)
/// Retourne le nombre de lignes affectées
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_execute(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe {
        if db_ptr == 0 || query_ptr == 0 {
            eprintln!("[MySQL] Error: null parameter in execute");
            return 0;
        }
    
        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        let query = ptr_to_str(query_ptr);
    
        let pool = db.pool.lock().unwrap();
        let mut conn = match pool.get_conn() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[MySQL] Failed to get connection: {}", e);
                return 0;
            }
        };
    
        match conn.query_drop(&query) {
            Ok(_) => {
                let affected = conn.affected_rows();
                let last_id = conn.last_insert_id();
                
                *db.affected_rows.lock().unwrap() = affected as i64;
                *db.last_insert_id.lock().unwrap() = last_id as i64;
                
                affected as i64
            }
            Err(e) => {
                eprintln!("[MySQL] Execute error: {}", e);
                0
            }
        }
    }
}

/// db.query(query) → array<map<string, mixed>>
/// Exécute une requête SELECT et retourne toutes les lignes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_query(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe {
        if db_ptr == 0 || query_ptr == 0 {
            eprintln!("[MySQL] Error: null parameter in query");
            return crate::__array_new();
        }
    
        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        let query = ptr_to_str(query_ptr);
    
        let pool = db.pool.lock().unwrap();
        let mut conn = match pool.get_conn() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[MySQL] Failed to get connection: {}", e);
                return crate::__array_new();
            }
        };
    
        let result_array = crate::__array_new();
    
        match conn.query_iter(&query) {
            Ok(result) => {
                for row_result in result {
                    match row_result {
                        Ok(row) => {
                            let row_map = crate::__map_new();
                            let columns = row.columns();
                            
                            for (i, column) in columns.iter().enumerate() {
                                let col_name = column.name_str();
                                let key_ptr = alloc_str(col_name.as_ref());
                                
                                let value: i64 = match row.get_opt(i) {
                                    Some(Ok(mysql::Value::NULL)) => 0,
                                    Some(Ok(mysql::Value::Int(v))) => v,
                                    Some(Ok(mysql::Value::UInt(v))) => v as i64,
                                    Some(Ok(mysql::Value::Float(v))) => crate::__box_float((v as f64).to_bits() as i64),
                                    Some(Ok(mysql::Value::Double(v))) => crate::__box_float(v.to_bits() as i64),
                                    Some(Ok(mysql::Value::Bytes(ref b))) => {
                                        if let Ok(s) = std::str::from_utf8(b) {
                                            alloc_str(s)
                                        } else {
                                            0
                                        }
                                    }
                                    _ => 0,
                                };
                                
                                crate::__map_set(row_map, key_ptr, value);
                            }
                            crate::__array_push(result_array, row_map);
                        }
                        Err(e) => {
                            eprintln!("[MySQL] Row error: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[MySQL] Query error: {}", e);
            }
        }
    
        result_array
    }
}

/// db.queryOne(query) → map<string, mixed>|null
/// Exécute une requête SELECT et retourne la première ligne ou null
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_queryOne(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe {
        if db_ptr == 0 || query_ptr == 0 {
            eprintln!("[MySQL] Error: null parameter in queryOne");
            return 0;
        }
    
        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        let query = ptr_to_str(query_ptr);
    
        let pool = db.pool.lock().unwrap();
        let mut conn = match pool.get_conn() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[MySQL] Failed to get connection: {}", e);
                return 0;
            }
        };
    
        match conn.query_iter(&query) {
            Ok(mut result) => {
                if let Some(Ok(row)) = result.next() {
                    let row_map = crate::__map_new();
                    let columns = row.columns();
                    
                    for (i, column) in columns.iter().enumerate() {
                        let col_name = column.name_str();
                        let key_ptr = alloc_str(col_name.as_ref());
                        
                        let value: i64 = match row.get_opt(i) {
                            Some(Ok(mysql::Value::NULL)) => 0,
                            Some(Ok(mysql::Value::Int(v))) => v,
                            Some(Ok(mysql::Value::UInt(v))) => v as i64,
                            Some(Ok(mysql::Value::Float(v))) => crate::__box_float((v as f64).to_bits() as i64),
                            Some(Ok(mysql::Value::Double(v))) => crate::__box_float(v.to_bits() as i64),
                            Some(Ok(mysql::Value::Bytes(ref b))) => {
                                if let Ok(s) = std::str::from_utf8(b) {
                                    alloc_str(s)
                                } else {
                                    0
                                }
                            }
                            _ => 0,
                        };
                        
                        crate::__map_set(row_map, key_ptr, value);
                    }
                    row_map
                } else {
                    0
                }
            }
            Err(e) => {
                eprintln!("[MySQL] QueryOne error: {}", e);
                0
            }
        }
    }
}

/// db.lastInsertId() → int
/// Retourne l'ID de la dernière insertion
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_lastInsertId(db_ptr: i64) -> i64 {
    unsafe {
        if db_ptr == 0 {
            return 0;
        }
        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        *db.last_insert_id.lock().unwrap()
    }
}

/// db.affectedRows() → int
/// Retourne le nombre de lignes affectées par la dernière opération
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_affectedRows(db_ptr: i64) -> i64 {
    unsafe {
        if db_ptr == 0 {
            return 0;
        }
        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        *db.affected_rows.lock().unwrap()
    }
}

/// db.close() → void
/// Ferme la connexion à la base de données
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_close(db_ptr: i64) {
    unsafe {
        if db_ptr == 0 {
            return;
        }
        // Le pool sera automatiquement fermé quand la structure est drop
        let _ = Box::from_raw(db_ptr as *mut OcaraMySQLDatabase);
    }
}

// MariaDB est un alias pour MySQL - mêmes fonctions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_connect(
    host_ptr: i64,
    user_ptr: i64,
    password_ptr: i64,
    database_ptr: i64,
) -> i64 {
    unsafe { MySQL_connect(host_ptr, user_ptr, password_ptr, database_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_execute(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { MySQL_execute(db_ptr, query_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_query(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { MySQL_query(db_ptr, query_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_queryOne(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { MySQL_queryOne(db_ptr, query_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_lastInsertId(db_ptr: i64) -> i64 {
    unsafe { MySQL_lastInsertId(db_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_affectedRows(db_ptr: i64) -> i64 {
    unsafe { MySQL_affectedRows(db_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_close(db_ptr: i64) {
    unsafe { MySQL_close(db_ptr) }
}
