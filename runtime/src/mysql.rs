// ─────────────────────────────────────────────────────────────────────────────
// ocara.MySQL / ocara.MariaDB — Base de données MySQL/MariaDB
//
// Fonctions exportées (convention C) :
//
//   MySQL_connect(host, user, password, database) → i64  // pointeur vers OcaraMySQLDatabase
//   MySQL_withConnect(host, user, password, database, f)  // connect+f(db)+close garanti
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
use crate::exception::throw_mysql_exception;

// Codes d'erreur MySQLException (voir docs/builtins/MySQL.md)
const ERR_CONNECT: i64 = 101;
const ERR_EXECUTE: i64 = 102;
const ERR_QUERY: i64 = 103;

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
            throw_mysql_exception(
                "Missing connection parameter (host/user/password/database)",
                ERR_CONNECT,
                "MySQL"
            );
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
                throw_mysql_exception(
                    &format!("Failed to connect to database '{}': {}", database, e),
                    ERR_CONNECT,
                    "MySQL"
                );
            }
        }
    }
}

/// MySQL::withConnect(host, user, password, database, f:Function<void(MySQL)>) → void
/// Connecte, exécute `f(db)`, ferme SYSTÉMATIQUEMENT — y compris si `f()`
/// lève une exception. Même patron que `SQLite::withOpen`
/// (`runtime/src/sqlite.rs`) et `Mutex::withLock` (`runtime/src/mutex.rs`) —
/// voir docs/roadmap.d/exceptions-setjmp-longjmp-dette.md.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_withConnect(
    host_ptr: i64,
    user_ptr: i64,
    password_ptr: i64,
    database_ptr: i64,
    fat_ptr: i64,
) {
    unsafe {
        let db_ptr = MySQL_connect(host_ptr, user_ptr, password_ptr, database_ptr);

        let func_ptr = *(fat_ptr as *const i64);
        let env_ptr  = *((fat_ptr as *const i64).add(1));

        let outcome = crate::run_closure_catching_with_arg(func_ptr, env_ptr, db_ptr);

        // Fermeture inconditionnelle — succès ou exception.
        MySQL_close(db_ptr);

        if let Err((error_val, error_type)) = outcome {
            crate::__ocara_fail(error_val, error_type);
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
            throw_mysql_exception("Missing parameter in execute", ERR_EXECUTE, "MySQL");
        }

        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        let query = ptr_to_str(query_ptr);

        // Le verrou (`MutexGuard<Pool>`) est entièrement scopé à cette
        // closure et se relâche normalement (Drop) à sa sortie, qu'elle
        // réussisse ou échoue — AVANT tout throw_mysql_exception, qui saute
        // par `longjmp` et ne laisserait jamais ce `Drop` s'exécuter s'il
        // restait à faire à ce moment-là : verrou tenu indéfiniment,
        // deadlock (même mécanisme que pour SQLite, voir
        // docs/roadmap.d/memoire-deadlocks-raise.md).
        let result: Result<(i64, u64), String> = (|| {
            let pool = db.pool.lock().unwrap();
            let mut conn = pool.get_conn()
                .map_err(|e| format!("Failed to get connection: {}", e))?;
            conn.query_drop(&query)
                .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;
            Ok((conn.affected_rows() as i64, conn.last_insert_id()))
        })();

        match result {
            Ok((affected, last_id)) => {
                *db.affected_rows.lock().unwrap() = affected;
                *db.last_insert_id.lock().unwrap() = last_id as i64;
                affected
            }
            Err(msg) => throw_mysql_exception(&msg, ERR_EXECUTE, "MySQL"),
        }
    }
}

/// db.query(query) → array<map<string, mixed>>
/// Exécute une requête SELECT et retourne toutes les lignes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_query(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe {
        if db_ptr == 0 || query_ptr == 0 {
            throw_mysql_exception("Missing parameter in query", ERR_QUERY, "MySQL");
        }

        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        let query = ptr_to_str(query_ptr);

        // Voir le commentaire de MySQL_execute : le verrou est entièrement
        // scopé à cette closure et se relâche normalement à sa sortie, avant
        // tout throw_mysql_exception.
        let result: Result<i64, String> = (|| {
            let pool = db.pool.lock().unwrap();
            let mut conn = pool.get_conn()
                .map_err(|e| format!("Failed to get connection: {}", e))?;

            let result_array = crate::__array_new();

            let query_result = conn.query_iter(&query)
                .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;

            for row_result in query_result {
                let row = row_result
                    .map_err(|e| format!("Failed to read row for query '{}': {}", query, e))?;
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

            Ok(result_array)
        })();

        match result {
            Ok(result_array) => result_array,
            Err(msg) => throw_mysql_exception(&msg, ERR_QUERY, "MySQL"),
        }
    }
}

/// db.queryOne(query) → map<string, mixed>|null
/// Exécute une requête SELECT et retourne la première ligne ou null
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_queryOne(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe {
        if db_ptr == 0 || query_ptr == 0 {
            throw_mysql_exception("Missing parameter in queryOne", ERR_QUERY, "MySQL");
        }

        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        let query = ptr_to_str(query_ptr);

        // Voir le commentaire de MySQL_execute.
        let result: Result<i64, String> = (|| {
            let pool = db.pool.lock().unwrap();
            let mut conn = pool.get_conn()
                .map_err(|e| format!("Failed to get connection: {}", e))?;

            let mut query_result = conn.query_iter(&query)
                .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;

            if let Some(Ok(row)) = query_result.next() {
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
                Ok(row_map)
            } else {
                Ok(0)
            }
        })();

        match result {
            Ok(row_map) => row_map,
            Err(msg) => throw_mysql_exception(&msg, ERR_QUERY, "MySQL"),
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
pub unsafe extern "C" fn MariaDB_withConnect(
    host_ptr: i64,
    user_ptr: i64,
    password_ptr: i64,
    database_ptr: i64,
    fat_ptr: i64,
) {
    unsafe { MySQL_withConnect(host_ptr, user_ptr, password_ptr, database_ptr, fat_ptr) }
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
