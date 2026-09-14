// ─────────────────────────────────────────────────────────────────────────────
// ocara.SQLite — Base de données SQLite
//
// Fonctions exportées (convention C) :
//
//   SQLite_open(path_ptr)              → i64  // Ouvre/crée une base et retourne un pointeur
//   SQLite_execute(self_ptr, query_ptr) → void // Exécute une requête SQL
//   SQLite_query(self_ptr, query_ptr)  → i64  // Exécute SELECT et retourne array de maps
//   SQLite_queryOne(self_ptr, query_ptr) → i64  // Exécute SELECT et retourne une map
//   SQLite_lastInsertId(self_ptr)      → i64  // Retourne l'ID de la dernière insertion
//   SQLite_affectedRows(self_ptr)      → i64  // Retourne le nombre de lignes affectées
//   SQLite_close(self_ptr)             → void // Ferme la connexion
//
// Gestion d'erreurs : Les fonctions lèvent SQLiteException en cas d'erreur.
//
// Codes d'erreur SQLiteException :
//   101 - OPEN         : Erreur d'ouverture de la base de données
//   102 - EXECUTE      : Erreur d'exécution d'une requête
//   103 - QUERY        : Erreur d'exécution d'un SELECT
//   104 - CLOSE        : Erreur de fermeture de la connexion
// ─────────────────────────────────────────────────────────────────────────────

use rusqlite::{Connection, params};
use std::sync::Mutex;
use crate::{alloc_str, ptr_to_str};
use crate::exception::throw_sqlite_exception;

// Codes d'erreur SQLiteException
const ERR_OPEN: i64 = 101;
const ERR_EXECUTE: i64 = 102;
const ERR_QUERY: i64 = 103;

/// Structure interne représentant une connexion SQLite
struct OcaraSQLiteDatabase {
    conn: Mutex<Connection>,
    last_insert_id: Mutex<i64>,
    affected_rows: Mutex<i64>,
}

/// SQLite::open(path:string) → SQLite
/// Ouvre ou crée une base de données SQLite
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_open(path_ptr: i64) -> i64 {
    unsafe {
        let path = ptr_to_str(path_ptr).to_string();
        
        match Connection::open(&path) {
            Ok(conn) => {
                let db = Box::new(OcaraSQLiteDatabase {
                    conn: Mutex::new(conn),
                    last_insert_id: Mutex::new(0),
                    affected_rows: Mutex::new(0),
                });
                Box::into_raw(db) as i64
            }
            Err(e) => {
                throw_sqlite_exception(
                    &format!("Failed to open database '{}': {}", path, e),
                    ERR_OPEN,
                    "SQLite"
                );
            }
        }
    }
}

/// db.execute(query:string) → void
/// Exécute une requête SQL (INSERT, UPDATE, DELETE, CREATE, etc.)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_execute(self_ptr: i64, query_ptr: i64) {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);
        let query = ptr_to_str(query_ptr).to_string();

        // Le verrou (`MutexGuard`) est entièrement scopé à cette closure et
        // se relâche normalement (Drop) à sa sortie, qu'elle réussisse ou
        // échoue — AVANT tout appel à throw_sqlite_exception, qui saute par
        // `longjmp` et ne laisserait jamais ce `Drop` s'exécuter s'il restait
        // à faire à ce moment-là : verrou tenu indéfiniment, deadlock
        // confirmé par reproduction sur la requête suivante (voir
        // docs/roadmap.d/memoire-deadlocks-raise.md). Tenter de `drop()`
        // explicitement le guard dans une branche `Err` en gardant la
        // connexion nommée pour l'autre se heurte au borrow-checker dès que
        // la valeur retournée (ici aucune, mais `Statement`/`Rows` dans
        // query/queryOne) porte la durée de vie de la connexion — cette
        // closure est la façon la plus simple de garantir l'ordre sans ça.
        let result: Result<(i64, Option<i64>), String> = (|| {
            let conn = db.conn.lock().unwrap();
            match conn.execute(&query, params![]) {
                Ok(affected) => {
                    let last_id = if query.trim().to_uppercase().starts_with("INSERT") {
                        Some(conn.last_insert_rowid())
                    } else {
                        None
                    };
                    Ok((affected as i64, last_id))
                }
                Err(e) => Err(format!("Failed to execute query '{}': {}", query, e)),
            }
        })();

        match result {
            Ok((affected, last_id)) => {
                *db.affected_rows.lock().unwrap() = affected;
                if let Some(id) = last_id {
                    *db.last_insert_id.lock().unwrap() = id;
                }
            }
            Err(msg) => throw_sqlite_exception(&msg, ERR_EXECUTE, "SQLite"),
        }
    }
}

/// db.query(query:string) → map<string, mixed>[]
/// Exécute un SELECT et retourne un array de maps
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_query(self_ptr: i64, query_ptr: i64) -> i64 {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);
        let query = ptr_to_str(query_ptr).to_string();

        // Voir le commentaire de SQLite_execute : le verrou est entièrement
        // scopé à cette closure (y compris `stmt`/`rows`, qui en empruntent
        // la durée de vie) et se relâche normalement à sa sortie, avant tout
        // throw_sqlite_exception.
        let result: Result<i64, String> = (|| {
            let mut conn_guard = db.conn.lock().unwrap();
            let conn = &mut *conn_guard;

            let mut stmt = conn.prepare(&query)
                .map_err(|e| format!("Failed to prepare query '{}': {}", query, e))?;

            let column_count = stmt.column_count();
            let column_names: Vec<String> = (0..column_count)
                .map(|i| stmt.column_name(i).unwrap().to_string())
                .collect();

            let mut rows = stmt.query(params![])
                .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;

            // Créer un array Ocara pour stocker les résultats
            let result_array = crate::__array_new();

            while let Ok(Some(row)) = rows.next() {
                // Créer une map pour cette ligne
                let row_map = crate::__map_new();

                for (i, col_name) in column_names.iter().enumerate() {
                    let key = alloc_str(col_name);

                    // Essayer de lire différents types de valeurs
                    let value = if let Ok(v) = row.get::<_, i64>(i) {
                        v
                    } else if let Ok(v) = row.get::<_, f64>(i) {
                        // Convertir float en i64 (représentation bits)
                        v.to_bits() as i64
                    } else if let Ok(v) = row.get::<_, String>(i) {
                        alloc_str(&v)
                    } else {
                        // NULL ou type non supporté → stocker 0
                        0
                    };

                    crate::__map_set(row_map, key, value);
                }

                crate::__array_push(result_array, row_map);
            }

            *db.affected_rows.lock().unwrap() = crate::__array_len(result_array);
            Ok(result_array)
        })();

        match result {
            Ok(result_array) => result_array,
            Err(msg) => throw_sqlite_exception(&msg, ERR_QUERY, "SQLite"),
        }
    }
}

/// db.queryOne(query:string) → map<string, mixed>
/// Exécute un SELECT et retourne une seule ligne (ou map vide si aucun résultat)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_queryOne(self_ptr: i64, query_ptr: i64) -> i64 {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);
        let query = ptr_to_str(query_ptr).to_string();

        // Voir le commentaire de SQLite_execute.
        let result: Result<i64, String> = (|| {
            let mut conn_guard = db.conn.lock().unwrap();
            let conn = &mut *conn_guard;

            let mut stmt = conn.prepare(&query)
                .map_err(|e| format!("Failed to prepare query '{}': {}", query, e))?;

            let column_count = stmt.column_count();
            let column_names: Vec<String> = (0..column_count)
                .map(|i| stmt.column_name(i).unwrap().to_string())
                .collect();

            let mut rows = stmt.query(params![])
                .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;

            // Créer une map pour stocker le résultat
            let row_map = crate::__map_new();

            let count = if let Ok(Some(row)) = rows.next() {
                for (i, col_name) in column_names.iter().enumerate() {
                    let key = alloc_str(col_name);

                    // Essayer de lire différents types de valeurs
                    let value = if let Ok(v) = row.get::<_, i64>(i) {
                        v
                    } else if let Ok(v) = row.get::<_, f64>(i) {
                        // Convertir float en i64 (représentation bits)
                        v.to_bits() as i64
                    } else if let Ok(v) = row.get::<_, String>(i) {
                        alloc_str(&v)
                    } else {
                        // NULL ou type non supporté → stocker 0
                        0
                    };

                    crate::__map_set(row_map, key, value);
                }
                1
            } else {
                0
            };

            *db.affected_rows.lock().unwrap() = count;
            Ok(row_map)
        })();

        match result {
            Ok(row_map) => row_map,
            Err(msg) => throw_sqlite_exception(&msg, ERR_QUERY, "SQLite"),
        }
    }
}

/// db.lastInsertId() → int
/// Retourne l'ID de la dernière insertion
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_lastInsertId(self_ptr: i64) -> i64 {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);
        *db.last_insert_id.lock().unwrap()
    }
}

/// db.affectedRows() → int
/// Retourne le nombre de lignes affectées par la dernière requête
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_affectedRows(self_ptr: i64) -> i64 {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);
        *db.affected_rows.lock().unwrap()
    }
}

/// db.close() → void
/// Ferme la connexion à la base de données
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_close(self_ptr: i64) {
    unsafe {
        if self_ptr == 0 {
            return;
        }
        
        // Récupérer et détruire la Box
        let _ = Box::from_raw(self_ptr as *mut OcaraSQLiteDatabase);
        // Le drop automatique de Box fermera la connexion
    }
}
