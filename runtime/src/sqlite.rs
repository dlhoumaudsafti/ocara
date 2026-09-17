// ─────────────────────────────────────────────────────────────────────────────
// ocara.SQLite — Base de données SQLite
//
// Fonctions exportées (convention C) :
//
//   SQLite_open(path_ptr)              → i64  // Ouvre/crée une base et retourne un pointeur
//   SQLite_withOpen(path_ptr, f)       → void // open+f(db)+close garanti (même en cas de raise)
//
//   // One-shot, placeholders nominatifs `:nom` optionnels (map<string,mixed>|null) :
//   SQLite_execute_1(self_ptr, query_ptr)                              → void // sans binding, rétrocompatible
//   SQLite_execute_2(self_ptr, query_ptr, placeholder_ptr)             → void // avec binding, close=false
//   SQLite_execute(self_ptr, query_ptr, placeholder_ptr, close)        → void // forme complète
//   SQLite_query_1(self_ptr, query_ptr)                                → i64
//   SQLite_query(self_ptr, query_ptr, placeholder_ptr)                 → i64  // array de maps
//   SQLite_queryOne_1(self_ptr, query_ptr)                             → i64
//   SQLite_queryOne(self_ptr, query_ptr, placeholder_ptr)              → i64  // map
//
//   // Stepped/transactionnel — voir docs/roadmap.d/stdlib-sqlite-requetes-parametrees-transactions.md :
//   SQLite_prepare(self_ptr, query_ptr)                → void // valide + mémorise la requête
//   SQLite_bind(self_ptr, placeholders_ptr)            → void // binds nominatifs `:nom` (map<string,mixed>)
//   SQLite_commit_0(self_ptr)                          → i64  // = commit(close=false)
//   SQLite_commit(self_ptr, close)                     → i64  // exécute+commit ; mixed (array de maps SI SELECT, sinon int)
//   SQLite_rollback_0(self_ptr)                        → void // = rollback(close=false)
//   SQLite_rollback(self_ptr, close)                   → void // annule la transaction en cours (no-op si aucune)
//
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
//   104 - CLOSE        : Erreur de fermeture de la connexion (réservé, non utilisé actuellement)
//   105 - PREPARE      : Erreur de syntaxe/préparation d'une requête
//   106 - BIND         : Placeholder inconnu, type de valeur non bindable, ou bind() sans prepare()
//   107 - COMMIT       : Erreur pendant BEGIN/exécution/COMMIT, ou commit() sans prepare()
//
// rollback() n'a volontairement PAS de code d'erreur dédié : un échec du
// ROLLBACK lui-même est absorbé silencieusement (best-effort, voir
// SQLite_rollback) pour ne jamais masquer l'exception d'origine qui a mené à
// l'appeler.
// ─────────────────────────────────────────────────────────────────────────────

use rusqlite::{Connection, Statement, ToSql};
use rusqlite::types::Value as SqlValue;
use std::sync::Mutex;
use crate::{alloc_str, ptr_to_str};
use crate::exception::throw_sqlite_exception;

// Codes d'erreur SQLiteException
const ERR_OPEN: i64 = 101;
const ERR_EXECUTE: i64 = 102;
const ERR_QUERY: i64 = 103;
const ERR_PREPARE: i64 = 105;
const ERR_BIND: i64 = 106;
const ERR_COMMIT: i64 = 107;

/// Structure interne représentant une connexion SQLite
struct OcaraSQLiteDatabase {
    conn: Mutex<Connection>,
    last_insert_id: Mutex<i64>,
    affected_rows: Mutex<i64>,
    // ── État du flux stepped prepare()/bind()/commit()/rollback() ──────────
    // Posé par `prepare()`, consommé par `commit()`/`rollback()`. Volontairement
    // pas de `rusqlite::Statement` gardée vivante entre les appels FFI (elle
    // emprunterait `conn` sur une durée de vie qui ne survivrait pas à un appel
    // C séparé) — on ne garde que le texte + les binds déjà convertis, et
    // `commit()` re-prépare au moment de l'exécution.
    pending_query: Mutex<Option<String>>,
    pending_binds: Mutex<Option<Vec<(String, SqlValue)>>>,
    // `true` entre un `BEGIN` réussi et le `COMMIT`/`ROLLBACK` qui le referme —
    // permet à `rollback()` de savoir s'il y a réellement quelque chose à
    // annuler côté SQL, plutôt que de tenter un `ROLLBACK` à l'aveugle.
    tx_open: Mutex<bool>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversion mixed <-> valeur SQL bindable
// ─────────────────────────────────────────────────────────────────────────────

/// Convertit une valeur `mixed` (i64 brut/boxé, voir `runtime/src/lib.rs`) en
/// `rusqlite::types::Value` bindable. `null` (0) → `SqlValue::Null`. Un
/// `int`/`float`/`bool` logé dans un `mixed` est TOUJOURS boxé dès qu'il vaut
/// `0` ou dépasse `PTR_THRESHOLD` (voir `box_int_if_needed` dans
/// `runtime/src/lib.rs`) — les cases `is_*_box` ci-dessous couvrent donc tous
/// les cas ambigus ; un `val` brut restant qui n'est ni boxé ni `0` est un
/// petit entier positif authentique (jamais un bool, toujours boxé sans
/// condition) ou un pointeur string/array/map/objet, distingué par
/// `get_value_type`. array/map/objet/function ne sont pas bindables : erreur
/// explicite plutôt qu'un comportement silencieux incorrect.
pub(crate) fn mixed_to_sql_value(val: i64) -> Result<SqlValue, String> {
    if val == 0 {
        return Ok(SqlValue::Null);
    }
    if crate::is_float_box(val) {
        return Ok(SqlValue::Real(unsafe { crate::unbox_float(val) }));
    }
    if crate::is_bool_box(val) {
        return Ok(SqlValue::Integer(if unsafe { crate::unbox_bool(val) } { 1 } else { 0 }));
    }
    if crate::is_int_box(val) {
        return Ok(SqlValue::Integer(unsafe { crate::unbox_int(val) }));
    }
    match crate::get_value_type(val) {
        1 => Ok(SqlValue::Integer(val)),                               // petit entier brut, jamais boxé
        4 => Ok(SqlValue::Text(unsafe { ptr_to_str(val) }.to_string())), // string
        other => Err(format!(
            "placeholder value of unsupported type (code {}) — only int/float/bool/string/null can be bound",
            other
        )),
    }
}

/// Lit une `map<string, mixed>` Ocara (`placeholders_ptr`, 0 = absente) et la
/// convertit en paires `(":nom", valeur SQL)` prêtes à binder. Le `:` est
/// ajouté ici — le code appelant Ocara écrit `bind({"id": 156})` (clé nue),
/// la requête SQL écrit `:id` (avec le préfixe), c'est ce point qui fait le
/// lien entre les deux conventions.
pub(crate) unsafe fn read_placeholders(placeholders_ptr: i64) -> Result<Vec<(String, SqlValue)>, String> {
    if placeholders_ptr == 0 {
        return Ok(Vec::new());
    }
    let entries = unsafe { crate::map_entries(placeholders_ptr) };
    let mut out = Vec::with_capacity(entries.len());
    for (key, raw_val) in entries {
        let sql_val = mixed_to_sql_value(raw_val)
            .map_err(|e| format!("bind placeholder ':{}': {}", key, e))?;
        out.push((format!(":{}", key), sql_val));
    }
    Ok(out)
}

/// `&[(&str, &dyn ToSql)]` — forme attendue par `rusqlite` pour un binding
/// nominatif (`Statement::query`/`Connection::execute` avec des paires
/// `(":nom", &valeur)`).
fn named_params(binds: &[(String, SqlValue)]) -> Vec<(&str, &dyn ToSql)> {
    binds.iter().map(|(k, v)| (k.as_str(), v as &dyn ToSql)).collect()
}

/// Lit toutes les lignes du `Statement` déjà bindé dans un `array<map<string,
/// mixed>>` Ocara — factorise la logique déjà utilisée par `query`/`queryOne`
/// one-shot et par `commit()` quand la requête préparée est un SELECT.
fn collect_all_rows(stmt: &mut Statement, binds: &[(&str, &dyn ToSql)]) -> Result<i64, String> {
    let column_count = stmt.column_count();
    let column_names: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).unwrap().to_string())
        .collect();

    let mut rows = stmt.query(&binds[..])
        .map_err(|e| format!("failed to execute query: {}", e))?;

    let result_array = crate::__array_new();
    while let Ok(Some(row)) = rows.next() {
        let row_map = crate::__map_new();
        for (i, col_name) in column_names.iter().enumerate() {
            let key = unsafe { alloc_str(col_name) };
            let value = if let Ok(v) = row.get::<_, i64>(i) {
                v
            } else if let Ok(v) = row.get::<_, f64>(i) {
                v.to_bits() as i64
            } else if let Ok(v) = row.get::<_, String>(i) {
                unsafe { alloc_str(&v) }
            } else {
                0
            };
            crate::__map_set(row_map, key, value);
        }
        crate::__array_push(result_array, row_map);
    }
    Ok(result_array)
}

// ─────────────────────────────────────────────────────────────────────────────
// SQLite::open / SQLite::withOpen
// ─────────────────────────────────────────────────────────────────────────────

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
                    pending_query: Mutex::new(None),
                    pending_binds: Mutex::new(None),
                    tx_open: Mutex::new(false),
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

/// SQLite::withOpen(path:string, f:Function<void(SQLite)>) → void
/// Ouvre la base, exécute `f(db)`, ferme SYSTÉMATIQUEMENT — y compris si
/// `f()` lève une exception. Voir docs/roadmap.d/exceptions-setjmp-longjmp-dette.md :
/// `SQLite::open(path)` seul laisse `.close()` à la charge du développeur —
/// un `raise` avant que `.close()` ne soit atteint (setjmp/longjmp, pas
/// d'unwinding Rust) fuit la connexion pour toujours (voir le diagnostic W04,
/// `src/sema/resource_raise.rs`). Même patron que `Mutex::withLock`
/// (`runtime/src/mutex.rs`), étendu pour une closure qui REÇOIT la ressource
/// en paramètre (`nameless(db:SQLite): void {...}`) plutôt que de la
/// capturer (la connexion n'existe pas encore avant cet appel) — voir
/// `run_closure_catching_with_arg`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_withOpen(path_ptr: i64, fat_ptr: i64) {
    unsafe {
        let path = ptr_to_str(path_ptr).to_string();

        let db_ptr = match Connection::open(&path) {
            Ok(conn) => {
                let db = Box::new(OcaraSQLiteDatabase {
                    conn: Mutex::new(conn),
                    last_insert_id: Mutex::new(0),
                    affected_rows: Mutex::new(0),
                    pending_query: Mutex::new(None),
                    pending_binds: Mutex::new(None),
                    tx_open: Mutex::new(false),
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
        };

        let func_ptr = *(fat_ptr as *const i64);
        let env_ptr  = *((fat_ptr as *const i64).add(1));

        let outcome = crate::run_closure_catching_with_arg(func_ptr, env_ptr, db_ptr);

        // Fermeture inconditionnelle — succès ou exception, c'est tout
        // l'intérêt de withOpen() par rapport à open()/close() manuels.
        let _ = Box::from_raw(db_ptr as *mut OcaraSQLiteDatabase);

        if let Err((error_val, error_type)) = outcome {
            // Relancer l'exception d'origine vers l'appelant, maintenant que
            // la connexion est fermée.
            crate::__ocara_fail(error_val, error_type);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// db.execute — one-shot, avec binding nominatif optionnel
// ─────────────────────────────────────────────────────────────────────────────

/// db.execute(query:string) → void — rétrocompatible, sans binding.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_execute_1(self_ptr: i64, query_ptr: i64) {
    unsafe { SQLite_execute(self_ptr, query_ptr, 0, 0) }
}

/// db.execute(query:string, placeholder:map<string,mixed>|null) → void — close=false.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_execute_2(self_ptr: i64, query_ptr: i64, placeholder_ptr: i64) {
    unsafe { SQLite_execute(self_ptr, query_ptr, placeholder_ptr, 0) }
}

/// db.execute(query:string, placeholder:map<string,mixed>|null = null, close:bool = false) → void
/// Exécute une requête SQL (INSERT, UPDATE, DELETE, CREATE, etc.), avec
/// binding nominatif `:nom` optionnel. Ferme la connexion si `close`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_execute(self_ptr: i64, query_ptr: i64, placeholder_ptr: i64, close: i64) {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);
        let query = ptr_to_str(query_ptr).to_string();

        // Le verrou (`MutexGuard`) est entièrement scopé à cette closure et
        // se relâche normalement (Drop) à sa sortie, qu'elle réussisse ou
        // échoue — AVANT tout appel à throw_sqlite_exception, qui saute par
        // `longjmp` et ne laisserait jamais ce `Drop` s'exécuter s'il restait
        // à faire à ce moment-là : verrou tenu indéfiniment, deadlock
        // confirmé par reproduction sur la requête suivante (voir
        // docs/roadmap.d/memoire-deadlocks-raise.md).
        let result: Result<(i64, Option<i64>), String> = (|| {
            let binds = read_placeholders(placeholder_ptr)?;
            let named = named_params(&binds);
            let conn = db.conn.lock().unwrap();
            match conn.execute(&query, &named[..]) {
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
                if close != 0 {
                    SQLite_close(self_ptr);
                }
            }
            Err(msg) => throw_sqlite_exception(&msg, ERR_EXECUTE, "SQLite"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// db.query / db.queryOne — one-shot, avec binding nominatif optionnel
// ─────────────────────────────────────────────────────────────────────────────

/// db.query(query:string) → map<string, mixed>[] — rétrocompatible, sans binding.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_query_1(self_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { SQLite_query(self_ptr, query_ptr, 0) }
}

/// db.query(query:string, placeholder:map<string,mixed>|null = null) → map<string, mixed>[]
/// Exécute un SELECT et retourne un array de maps
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_query(self_ptr: i64, query_ptr: i64, placeholder_ptr: i64) -> i64 {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);
        let query = ptr_to_str(query_ptr).to_string();

        // Voir le commentaire de SQLite_execute : le verrou est entièrement
        // scopé à cette closure (y compris `stmt`/`rows`, qui en empruntent
        // la durée de vie) et se relâche normalement à sa sortie, avant tout
        // throw_sqlite_exception.
        let result: Result<i64, String> = (|| {
            let binds = read_placeholders(placeholder_ptr)?;
            let named = named_params(&binds);
            let mut conn_guard = db.conn.lock().unwrap();
            let conn = &mut *conn_guard;

            let mut stmt = conn.prepare(&query)
                .map_err(|e| format!("Failed to prepare query '{}': {}", query, e))?;

            let result_array = collect_all_rows(&mut stmt, &named)?;
            *db.affected_rows.lock().unwrap() = crate::__array_len(result_array);
            Ok(result_array)
        })();

        match result {
            Ok(result_array) => result_array,
            Err(msg) => throw_sqlite_exception(&msg, ERR_QUERY, "SQLite"),
        }
    }
}

/// db.queryOne(query:string) → map<string, mixed> — rétrocompatible, sans binding.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_queryOne_1(self_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { SQLite_queryOne(self_ptr, query_ptr, 0) }
}

/// db.queryOne(query:string, placeholder:map<string,mixed>|null = null) → map<string, mixed>
/// Exécute un SELECT et retourne une seule ligne (ou map vide si aucun résultat)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_queryOne(self_ptr: i64, query_ptr: i64, placeholder_ptr: i64) -> i64 {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);
        let query = ptr_to_str(query_ptr).to_string();

        // Voir le commentaire de SQLite_execute.
        let result: Result<i64, String> = (|| {
            let binds = read_placeholders(placeholder_ptr)?;
            let named = named_params(&binds);
            let mut conn_guard = db.conn.lock().unwrap();
            let conn = &mut *conn_guard;

            let mut stmt = conn.prepare(&query)
                .map_err(|e| format!("Failed to prepare query '{}': {}", query, e))?;

            let column_count = stmt.column_count();
            let column_names: Vec<String> = (0..column_count)
                .map(|i| stmt.column_name(i).unwrap().to_string())
                .collect();

            let mut rows = stmt.query(&named[..])
                .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;

            let row_map = crate::__map_new();

            let count = if let Ok(Some(row)) = rows.next() {
                for (i, col_name) in column_names.iter().enumerate() {
                    let key = alloc_str(col_name);
                    let value = if let Ok(v) = row.get::<_, i64>(i) {
                        v
                    } else if let Ok(v) = row.get::<_, f64>(i) {
                        v.to_bits() as i64
                    } else if let Ok(v) = row.get::<_, String>(i) {
                        alloc_str(&v)
                    } else {
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

// ─────────────────────────────────────────────────────────────────────────────
// prepare / bind / commit / rollback — flux stepped transactionnel
// ─────────────────────────────────────────────────────────────────────────────

/// db.prepare(query:string) → void
/// Valide la requête immédiatement (compilation SQLite — erreur de syntaxe
/// remontée tout de suite, sans attendre `commit()`) puis la mémorise pour
/// `bind()`/`commit()`. N'exécute rien ici — la `Statement` de validation est
/// relâchée dès cette fonction terminée, `commit()` re-préparera au moment de
/// l'exécution avec les binds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_prepare(self_ptr: i64, query_ptr: i64) {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);
        let query = ptr_to_str(query_ptr).to_string();

        let result: Result<(), String> = (|| {
            let conn = db.conn.lock().unwrap();
            conn.prepare(&query)
                .map_err(|e| format!("Failed to prepare query '{}': {}", query, e))?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                *db.pending_query.lock().unwrap() = Some(query);
                *db.pending_binds.lock().unwrap() = None;
            }
            Err(msg) => throw_sqlite_exception(&msg, ERR_PREPARE, "SQLite"),
        }
    }
}

/// db.bind(placeholders:map<string, mixed>) → void
/// Bind les placeholders nominatifs `:nom` de la requête posée par le dernier
/// `prepare()`. Doit suivre un `prepare()` sans `commit()`/`rollback()` entre
/// les deux.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_bind(self_ptr: i64, placeholders_ptr: i64) {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);

        if db.pending_query.lock().unwrap().is_none() {
            throw_sqlite_exception(
                "bind() called without a pending prepare() — call prepare(query) first",
                ERR_BIND,
                "SQLite"
            );
        }

        match read_placeholders(placeholders_ptr) {
            Ok(binds) => *db.pending_binds.lock().unwrap() = Some(binds),
            Err(msg) => throw_sqlite_exception(&msg, ERR_BIND, "SQLite"),
        }
    }
}

/// db.commit() → mixed — équivalent à commit(close=false).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_commit_0(self_ptr: i64) -> i64 {
    unsafe { SQLite_commit(self_ptr, 0) }
}

/// db.commit(close:bool = false) → mixed
/// Exécute la requête posée par `prepare()`/`bind()` à l'intérieur d'une
/// transaction SQL (`BEGIN` ... `COMMIT`). Retourne `array<map<string,
/// mixed>>` si la requête produit des colonnes (SELECT), sinon `int` (lignes
/// affectées) — auto-détecté via `Statement::column_count()`. En cas
/// d'échec à n'importe quelle étape (BEGIN/prepare/bind/exécution/COMMIT),
/// lève SQLiteException SANS rollback automatique : l'état reste en place
/// pour que `rollback()`, appelé explicitement par l'appelant, nettoie —
/// voir le flux de référence dans docs/builtins/SQLite.md.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_commit(self_ptr: i64, close: i64) -> i64 {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);

        let query = match db.pending_query.lock().unwrap().clone() {
            Some(q) => q,
            None => throw_sqlite_exception(
                "commit() called without a pending prepare() — call prepare(query) first",
                ERR_COMMIT,
                "SQLite"
            ),
        };
        let binds = db.pending_binds.lock().unwrap().clone().unwrap_or_default();
        let named = named_params(&binds);

        // Voir le commentaire de SQLite_execute : le verrou reste scopé à
        // cette closure, relâché avant tout throw_sqlite_exception.
        let result: Result<i64, String> = (|| {
            let mut conn_guard = db.conn.lock().unwrap();
            let conn = &mut *conn_guard;

            conn.execute_batch("BEGIN")
                .map_err(|e| format!("Failed to BEGIN transaction: {}", e))?;
            *db.tx_open.lock().unwrap() = true;

            let mut stmt = conn.prepare(&query)
                .map_err(|e| format!("Failed to prepare query '{}': {}", query, e))?;

            let outcome: Result<i64, String> = if stmt.column_count() > 0 {
                // SELECT — collecte toutes les lignes.
                collect_all_rows(&mut stmt, &named)
            } else {
                // Écriture — exécute et boxe le nombre de lignes affectées
                // (voir `box_int_if_needed` : un int logé dans un `mixed`
                // doit être boxé s'il vaut 0 ou dépasse PTR_THRESHOLD).
                stmt.execute(&named[..])
                    .map(|affected| crate::box_int_if_needed(affected as i64))
                    .map_err(|e| format!("Failed to execute query '{}': {}", query, e))
            };
            let value = outcome?;

            conn.execute_batch("COMMIT")
                .map_err(|e| format!("Failed to COMMIT transaction: {}", e))?;

            Ok(value)
        })();

        match result {
            Ok(value) => {
                *db.pending_query.lock().unwrap() = None;
                *db.pending_binds.lock().unwrap() = None;
                *db.tx_open.lock().unwrap() = false;
                if close != 0 {
                    SQLite_close(self_ptr);
                }
                value
            }
            Err(msg) => throw_sqlite_exception(&msg, ERR_COMMIT, "SQLite"),
        }
    }
}

/// db.rollback() → void — équivalent à rollback(close=false).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_rollback_0(self_ptr: i64) {
    unsafe { SQLite_rollback(self_ptr, 0) }
}

/// db.rollback(close:bool = false) → void
/// Annule la transaction laissée ouverte par un `commit()` échoué, et efface
/// l'état `prepare()`/`bind()` en attente. No-op silencieux s'il n'y a rien à
/// annuler (aucune transaction SQL réellement ouverte) — pensé pour être
/// appelé sans risque depuis un `on e is SQLiteException` sans connaître
/// précisément à quelle étape l'échec a eu lieu. L'échec du `ROLLBACK`
/// lui-même (rare — ex. connexion déjà dans un état incohérent) est
/// best-effort : il ne masque pas l'exception d'origine derrière une
/// nouvelle, il est simplement ignoré.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLite_rollback(self_ptr: i64, close: i64) {
    unsafe {
        let db = &*(self_ptr as *const OcaraSQLiteDatabase);

        if *db.tx_open.lock().unwrap() {
            let conn = db.conn.lock().unwrap();
            let _ = conn.execute_batch("ROLLBACK"); // best-effort, voir doc ci-dessus
        }

        *db.pending_query.lock().unwrap() = None;
        *db.pending_binds.lock().unwrap() = None;
        *db.tx_open.lock().unwrap() = false;

        if close != 0 {
            SQLite_close(self_ptr);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// lastInsertId / affectedRows / close
// ─────────────────────────────────────────────────────────────────────────────

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
