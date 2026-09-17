// ─────────────────────────────────────────────────────────────────────────────
// ocara.MySQL / ocara.MariaDB — Base de données MySQL/MariaDB
//
// Fonctions exportées (convention C) :
//
//   MySQL_connect(host, user, password, database) → i64  // pointeur vers OcaraMySQLDatabase
//   MySQL_withConnect(host, user, password, database, f)  // connect+f(db)+close garanti
//
//   // One-shot, placeholders nominatifs `:nom` optionnels (map<string,mixed>|null) :
//   MySQL_execute_1(db_ptr, query_ptr)                              → i64  // sans binding, rétrocompatible
//   MySQL_execute_2(db_ptr, query_ptr, placeholder_ptr)             → i64  // avec binding, close=false
//   MySQL_execute(db_ptr, query_ptr, placeholder_ptr, close)        → i64  // forme complète
//   MySQL_query_1(db_ptr, query_ptr)                                → i64
//   MySQL_query(db_ptr, query_ptr, placeholder_ptr)                 → i64  // array de maps
//   MySQL_queryOne_1(db_ptr, query_ptr)                             → i64
//   MySQL_queryOne(db_ptr, query_ptr, placeholder_ptr)              → i64  // map ou 0 (null)
//
//   // Stepped/transactionnel — voir docs/roadmap.d/stdlib-mysql-requetes-parametrees-transactions.md :
//   MySQL_prepare(db_ptr, query_ptr)                → void // épingle une connexion + valide + mémorise la requête
//   MySQL_bind(db_ptr, placeholders_ptr)            → void // binds nominatifs `:nom` (map<string,mixed>)
//   MySQL_commit_0(db_ptr)                          → i64  // = commit(close=false)
//   MySQL_commit(db_ptr, close)                     → i64  // exécute+commit ; mixed (array de maps SI SELECT, sinon int)
//   MySQL_rollback_0(db_ptr)                        → void // = rollback(close=false)
//   MySQL_rollback(db_ptr, close)                   → void // annule la transaction en cours (no-op si aucune)
//
//   MySQL_lastInsertId(db_ptr)                     → i64
//   MySQL_affectedRows(db_ptr)                     → i64
//   MySQL_close(db_ptr)                            → void
//
// ── Préalable architectural (voir le ticket ci-dessus) ─────────────────────
// `execute`/`query`/`queryOne` one-shot continuent de piocher une connexion
// DIFFÉRENTE dans `pool` à chaque appel (`pool.get_conn()`, comportement
// inchangé) — aucune garantie transactionnelle nécessaire pour un appel
// isolé. Le flux stepped (`prepare`/`bind`/`commit`/`rollback`) épingle en
// revanche UNE connexion (`pinned_conn`) pour toute la durée du cycle : sans
// ça, le `START TRANSACTION` et le `COMMIT` pourraient atterrir sur deux
// connexions différentes du pool, donc dans deux sessions serveur sans
// transaction en commun.
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::Mutex;
use mysql::{Pool, PooledConn, OptsBuilder, Value as MyValue};
use mysql::prelude::*;
use crate::{alloc_str, ptr_to_str};
use crate::exception::throw_mysql_exception;

// Codes d'erreur MySQLException (voir docs/builtins/MySQL.md)
const ERR_CONNECT: i64 = 101;
const ERR_EXECUTE: i64 = 102;
const ERR_QUERY: i64 = 103;
const ERR_PREPARE: i64 = 105;
const ERR_BIND: i64 = 106;
const ERR_COMMIT: i64 = 107;

/// Structure interne représentant une connexion MySQL
pub struct OcaraMySQLDatabase {
    pool: Mutex<Pool>,
    last_insert_id: Mutex<i64>,
    affected_rows: Mutex<i64>,
    // ── État du flux stepped prepare()/bind()/commit()/rollback() ──────────
    pending_query: Mutex<Option<String>>,
    pending_binds: Mutex<Option<Vec<(String, MyValue)>>>,
    tx_open: Mutex<bool>,
    // Connexion épinglée pour la durée d'un cycle prepare→bind→commit/rollback
    // — voir le commentaire de tête de fichier.
    pinned_conn: Mutex<Option<PooledConn>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversion mixed <-> valeur MySQL bindable
// ─────────────────────────────────────────────────────────────────────────────

/// Même logique que `sqlite::mixed_to_sql_value` (voir sa doc), adaptée à
/// `mysql::Value`. MySQL n'a pas de type bool natif (convention TINYINT(1) :
/// un `bool` Ocara se bind comme un `Int(0/1)`, exactement comme le lit déjà
/// `MySQL_query`/`MySQL_queryOne` pour une colonne entière).
pub(crate) fn mixed_to_mysql_value(val: i64) -> Result<MyValue, String> {
    if val == 0 {
        return Ok(MyValue::NULL);
    }
    if crate::is_float_box(val) {
        return Ok(MyValue::Double(unsafe { crate::unbox_float(val) }));
    }
    if crate::is_bool_box(val) {
        return Ok(MyValue::Int(if unsafe { crate::unbox_bool(val) } { 1 } else { 0 }));
    }
    if crate::is_int_box(val) {
        return Ok(MyValue::Int(unsafe { crate::unbox_int(val) }));
    }
    match crate::get_value_type(val) {
        1 => Ok(MyValue::Int(val)),
        4 => Ok(MyValue::Bytes(unsafe { ptr_to_str(val) }.as_bytes().to_vec())),
        other => Err(format!(
            "placeholder value of unsupported type (code {}) — only int/float/bool/string/null can be bound",
            other
        )),
    }
}

/// Lit une `map<string, mixed>` Ocara et la convertit en paires `(nom, valeur
/// MySQL)`. Contrairement à SQLite (`sqlite::read_placeholders`), PAS de `:`
/// ajouté ici : le parseur de paramètres nommés du crate `mysql`
/// (`mysql_common::named_params`) extrait déjà les noms SANS le `:` de la
/// requête (`:id` → `"id"`) et attend les clés de `Params::Named` sous la
/// même forme nue — vérifié dans le code source vendored du crate avant
/// d'écrire cette fonction, pas supposé par symétrie avec SQLite.
pub(crate) unsafe fn read_placeholders(placeholders_ptr: i64) -> Result<Vec<(String, MyValue)>, String> {
    if placeholders_ptr == 0 {
        return Ok(Vec::new());
    }
    let entries = unsafe { crate::map_entries(placeholders_ptr) };
    let mut out = Vec::with_capacity(entries.len());
    for (key, raw_val) in entries {
        let v = mixed_to_mysql_value(raw_val)
            .map_err(|e| format!("bind placeholder ':{}': {}", key, e))?;
        out.push((key, v));
    }
    Ok(out)
}

/// `Vec::from` (voir `mysql_common::params`) construit TOUJOURS un
/// `Params::Named`, même à partir d'un `Vec` vide — or le crate `mysql`
/// rejette `Params::Named(_)`, y compris vide, contre une requête qui n'a
/// AUCUN placeholder nommé (`stmt.named_params` à `None`, classée "requête
/// positionnelle" en interne) : `DriverError { Can not pass named
/// parameters to positional query }`. Confirmé par reproduction avant ce
/// correctif — `MySQL_execute("DROP TABLE ...", null)` échouait alors que
/// la même requête réussissait très bien manuellement (`conn.prep(...)`
/// seul). Il faut donc explicitement `Params::Empty` quand `binds` est
/// vide, jamais `Params::from(vec![])`.
fn params_from_binds(binds: &[(String, MyValue)]) -> mysql::Params {
    if binds.is_empty() {
        mysql::Params::Empty
    } else {
        mysql::Params::from(binds.to_vec())
    }
}

/// Convertit une `mysql::Row` en `map<string, mixed>` Ocara — factorise la
/// lecture déjà utilisée par `query`/`queryOne` (one-shot et stepped).
///
/// Suppose que la ligne vient TOUJOURS du protocole BINAIRE (une requête
/// préparée, `exec_iter`/`exec_drop`/`Statement`) — jamais du protocole
/// TEXTE (`query_iter`/`query_drop`). C'est une vraie contrainte, pas un
/// détail : le protocole texte de MySQL/MariaDB encode TOUTE colonne en
/// `mysql::Value::Bytes` (texte), y compris les colonnes numériques — un
/// `INT` y est indiscernable d'un `VARCHAR` contenant la même chaîne de
/// chiffres. Ce `match` ne gère PAS ce cas : une valeur numérique lue via le
/// protocole texte tomberait dans la branche `Bytes` et reviendrait comme
/// une STRING Ocara, pas un `int`. Bug réel trouvé en écrivant les tests
/// unitaires Rust (`tests::mysql::mysql_execute_2_binds_named_placeholders`,
/// pas par les tests `.oc` de régression — invisible en interpolant dans un
/// template string, `${row["age"]}` affiche `"25"` que la valeur sous-jacente
/// soit l'entier `25` ou la chaîne `"25"`) : tout appelant de `MySQL_query_1`/
/// `MySQL_queryOne_1` passait auparavant par `query_iter` (protocole texte),
/// donc CHAQUE colonne numérique lue sans placeholder était en réalité une
/// string. Corrigé en supprimant ce chemin : `MySQL_execute`/`MySQL_query`/
/// `MySQL_queryOne` passent maintenant TOUJOURS par `exec_drop`/`exec_iter`
/// (protocole binaire), avec ou sans placeholder — voir leurs corps.
fn row_to_map(row: &mysql::Row) -> i64 {
    let row_map = crate::__map_new();
    let columns = row.columns();
    for (i, column) in columns.iter().enumerate() {
        let col_name = column.name_str();
        let key_ptr = unsafe { alloc_str(col_name.as_ref()) };
        let value: i64 = match row.as_ref(i) {
            Some(MyValue::NULL) => 0,
            Some(MyValue::Int(v)) => *v,
            Some(MyValue::UInt(v)) => *v as i64,
            Some(MyValue::Float(v)) => crate::__box_float((*v as f64).to_bits() as i64),
            Some(MyValue::Double(v)) => crate::__box_float(v.to_bits() as i64),
            Some(MyValue::Bytes(b)) => {
                if let Ok(s) = std::str::from_utf8(b) {
                    unsafe { alloc_str(s) }
                } else {
                    0
                }
            }
            _ => 0,
        };
        crate::__map_set(row_map, key_ptr, value);
    }
    row_map
}

// ─────────────────────────────────────────────────────────────────────────────
// MySQL::connect / MySQL::withConnect
// ─────────────────────────────────────────────────────────────────────────────

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
                    pending_query: Mutex::new(None),
                    pending_binds: Mutex::new(None),
                    tx_open: Mutex::new(false),
                    pinned_conn: Mutex::new(None),
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

// ─────────────────────────────────────────────────────────────────────────────
// db.execute — one-shot, avec binding nominatif optionnel
// ─────────────────────────────────────────────────────────────────────────────

/// db.execute(query:string) → int — rétrocompatible, sans binding.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_execute_1(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { MySQL_execute(db_ptr, query_ptr, 0, 0) }
}

/// db.execute(query:string, placeholder:map<string,mixed>|null) → int — close=false.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_execute_2(db_ptr: i64, query_ptr: i64, placeholder_ptr: i64) -> i64 {
    unsafe { MySQL_execute(db_ptr, query_ptr, placeholder_ptr, 0) }
}

/// db.execute(query:string, placeholder:map<string,mixed>|null = null, close:bool = false) → int
/// Exécute une requête SQL (INSERT, UPDATE, DELETE, CREATE, etc.), avec
/// binding nominatif `:nom` optionnel. Retourne le nombre de lignes
/// affectées. Ferme la connexion si `close`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_execute(db_ptr: i64, query_ptr: i64, placeholder_ptr: i64, close: i64) -> i64 {
    unsafe {
        if db_ptr == 0 || query_ptr == 0 {
            throw_mysql_exception("Missing parameter in execute", ERR_EXECUTE, "MySQL");
        }

        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        let query = ptr_to_str(query_ptr);

        // Voir le commentaire de `MySQL_execute` d'origine : le verrou est
        // entièrement scopé à cette closure, relâché avant tout
        // throw_mysql_exception (même raison que côté SQLite, voir
        // docs/roadmap.d/memoire-deadlocks-raise.md).
        let result: Result<(i64, u64), String> = (|| {
            let binds = read_placeholders(placeholder_ptr)?;
            let pool = db.pool.lock().unwrap();
            let mut conn = pool.get_conn()
                .map_err(|e| format!("Failed to get connection: {}", e))?;
            // Toujours `exec_drop` (protocole binaire, requête préparée),
            // jamais `query_drop` (protocole texte) — voir le commentaire
            // détaillé sur `row_to_map` : le protocole texte renvoie TOUTE
            // colonne en `Value::Bytes`, y compris les colonnes numériques,
            // ce qui n'affecte pas `execute()` (pas de lecture de colonnes
            // ici) mais casserait `query`/`queryOne` si un jour cette
            // fonction servait de modèle à un copier-coller.
            conn.exec_drop(query, params_from_binds(&binds))
                .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;
            Ok((conn.affected_rows() as i64, conn.last_insert_id()))
        })();

        match result {
            Ok((affected, last_id)) => {
                *db.affected_rows.lock().unwrap() = affected;
                *db.last_insert_id.lock().unwrap() = last_id as i64;
                if close != 0 {
                    MySQL_close(db_ptr);
                }
                affected
            }
            Err(msg) => throw_mysql_exception(&msg, ERR_EXECUTE, "MySQL"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// db.query / db.queryOne — one-shot, avec binding nominatif optionnel
// ─────────────────────────────────────────────────────────────────────────────

/// db.query(query:string) → array<map<string, mixed>> — rétrocompatible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_query_1(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { MySQL_query(db_ptr, query_ptr, 0) }
}

/// db.query(query:string, placeholder:map<string,mixed>|null = null) → array<map<string, mixed>>
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_query(db_ptr: i64, query_ptr: i64, placeholder_ptr: i64) -> i64 {
    unsafe {
        if db_ptr == 0 || query_ptr == 0 {
            throw_mysql_exception("Missing parameter in query", ERR_QUERY, "MySQL");
        }

        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        let query = ptr_to_str(query_ptr);

        let result: Result<i64, String> = (|| {
            let binds = read_placeholders(placeholder_ptr)?;
            let pool = db.pool.lock().unwrap();
            let mut conn = pool.get_conn()
                .map_err(|e| format!("Failed to get connection: {}", e))?;

            let result_array = crate::__array_new();

            // Toujours le protocole binaire (`exec_iter`, requête préparée) —
            // voir le commentaire détaillé de `row_to_map` : le protocole
            // texte (`query_iter`) renvoie toute colonne en `Value::Bytes`,
            // y compris les colonnes numériques (INT, DOUBLE...), que le
            // `match` de `row_to_map` distinguait auparavant de `Int`/
            // `Double` uniquement par CHANCE de protocole — bug réel trouvé
            // en écrivant les tests unitaires Rust (`tests::mysql`) :
            // `SELECT age FROM t` sans placeholder renvoyait la STRING "25"
            // au lieu de l'entier 25, invisible en interpolant dans un
            // template string (`${row["age"]}` affiche "25" dans les deux
            // cas) mais faux pour tout code faisant de l'arithmétique dessus.
            let query_result = conn.exec_iter(query, params_from_binds(&binds))
                .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;
            for row_result in query_result {
                let row = row_result
                    .map_err(|e| format!("Failed to read row for query '{}': {}", query, e))?;
                crate::__array_push(result_array, row_to_map(&row));
            }

            Ok(result_array)
        })();

        match result {
            Ok(result_array) => result_array,
            Err(msg) => throw_mysql_exception(&msg, ERR_QUERY, "MySQL"),
        }
    }
}

/// db.queryOne(query:string) → map<string, mixed>|null — rétrocompatible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_queryOne_1(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { MySQL_queryOne(db_ptr, query_ptr, 0) }
}

/// db.queryOne(query:string, placeholder:map<string,mixed>|null = null) → map<string, mixed>|null
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_queryOne(db_ptr: i64, query_ptr: i64, placeholder_ptr: i64) -> i64 {
    unsafe {
        if db_ptr == 0 || query_ptr == 0 {
            throw_mysql_exception("Missing parameter in queryOne", ERR_QUERY, "MySQL");
        }

        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        let query = ptr_to_str(query_ptr);

        let result: Result<i64, String> = (|| {
            let binds = read_placeholders(placeholder_ptr)?;
            let pool = db.pool.lock().unwrap();
            let mut conn = pool.get_conn()
                .map_err(|e| format!("Failed to get connection: {}", e))?;

            // `row_to_map` doit s'exécuter PENDANT que `query_result` est
            // encore en vie (`Row` lu en streaming depuis la connexion,
            // invalide une fois l'itérateur droppé) — bug réel trouvé en
            // testant contre un vrai serveur MariaDB : une première version
            // qui affectait `query_result.next()` à une variable AVANT de
            // sortir de ce bloc laissait tomber `query_result` en fin de
            // bloc, et chaque champ de la ligne revenait `null` (pas
            // d'erreur — `row.as_ref(i)` retournait silencieusement `None`).
            //
            // Toujours le protocole binaire (`exec_iter`) — voir le
            // commentaire détaillé de `row_to_map` sur le bug de typage du
            // protocole texte (`query_iter`) pour les colonnes numériques.
            let mut query_result = conn.exec_iter(query, params_from_binds(&binds))
                .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;
            match query_result.next() {
                Some(Ok(row)) => Ok(row_to_map(&row)),
                Some(Err(e)) => Err(format!("Failed to read row for query '{}': {}", query, e)),
                None => Ok(0),
            }
        })();

        match result {
            Ok(row_map) => row_map,
            Err(msg) => throw_mysql_exception(&msg, ERR_QUERY, "MySQL"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// prepare / bind / commit / rollback — flux stepped transactionnel
// ─────────────────────────────────────────────────────────────────────────────

/// Annule au mieux (best-effort) une transaction laissée ouverte par un
/// cycle précédent, sans jamais lever — appelée en tête de `prepare()` pour
/// ne JAMAIS rendre une connexion mid-transaction au pool (ce que ferait
/// silencieusement le remplacement de `pinned_conn` sinon), et par
/// `MySQL_rollback` elle-même.
unsafe fn rollback_pending(db: &OcaraMySQLDatabase) {
    let mut tx_open = db.tx_open.lock().unwrap();
    if *tx_open {
        if let Some(conn) = db.pinned_conn.lock().unwrap().as_mut() {
            let _ = conn.query_drop("ROLLBACK");
        }
    }
    *tx_open = false;
    *db.pending_query.lock().unwrap() = None;
    *db.pending_binds.lock().unwrap() = None;
    // La connexion épinglée retourne au pool ici (Drop de PooledConn).
    *db.pinned_conn.lock().unwrap() = None;
}

/// db.prepare(query:string) → void
/// Épingle une connexion dédiée au cycle (voir le commentaire de tête de
/// fichier), valide la requête immédiatement (`COM_STMT_PREPARE` réel côté
/// serveur — erreur de syntaxe remontée tout de suite) puis la mémorise pour
/// `bind()`/`commit()`. Un cycle précédent resté ouvert (commit() échoué,
/// rollback() jamais appelé) est annulé au préalable — voir
/// `rollback_pending`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_prepare(db_ptr: i64, query_ptr: i64) {
    unsafe {
        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        rollback_pending(db);

        let query = ptr_to_str(query_ptr).to_string();

        let result: Result<PooledConn, String> = (|| {
            let pool = db.pool.lock().unwrap();
            let mut conn = pool.get_conn()
                .map_err(|e| format!("Failed to get connection: {}", e))?;
            // `conn.prep` — prépare réellement côté serveur (COM_STMT_PREPARE),
            // donc valide la syntaxe ici. Mise en cache interne au crate
            // (`stmt_cache`, voir mysql-25.0.1/src/conn/mod.rs) : `commit()`
            // repassera la MÊME chaîne de requête à `exec_iter`/`exec_drop`,
            // qui réutilisera cette entrée plutôt que de re-préparer pour de
            // vrai — pas de fuite de handle côté serveur (contrairement à un
            // `prep()` ad-hoc jamais fermé sur un crate qui n'aurait pas ce
            // cache).
            conn.prep(&query)
                .map_err(|e| format!("Failed to prepare query '{}': {}", query, e))?;
            Ok(conn)
        })();

        match result {
            Ok(conn) => {
                *db.pinned_conn.lock().unwrap() = Some(conn);
                *db.pending_query.lock().unwrap() = Some(query);
                *db.pending_binds.lock().unwrap() = None;
            }
            Err(msg) => throw_mysql_exception(&msg, ERR_PREPARE, "MySQL"),
        }
    }
}

/// db.bind(placeholders:map<string, mixed>) → void
/// Bind les placeholders nominatifs `:nom` de la requête posée par le
/// dernier `prepare()`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_bind(db_ptr: i64, placeholders_ptr: i64) {
    unsafe {
        let db = &*(db_ptr as *const OcaraMySQLDatabase);

        if db.pending_query.lock().unwrap().is_none() {
            throw_mysql_exception(
                "bind() called without a pending prepare() — call prepare(query) first",
                ERR_BIND,
                "MySQL"
            );
        }

        match read_placeholders(placeholders_ptr) {
            Ok(binds) => *db.pending_binds.lock().unwrap() = Some(binds),
            Err(msg) => throw_mysql_exception(&msg, ERR_BIND, "MySQL"),
        }
    }
}

/// db.commit() → mixed — équivalent à commit(close=false).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_commit_0(db_ptr: i64) -> i64 {
    unsafe { MySQL_commit(db_ptr, 0) }
}

/// db.commit(close:bool = false) → mixed
/// Exécute la requête posée par `prepare()`/`bind()`, sur la connexion
/// épinglée par `prepare()`, à l'intérieur d'une transaction SQL
/// (`START TRANSACTION` ... `COMMIT`). Retourne `array<map<string,
/// mixed>>` si la requête produit des colonnes (SELECT), sinon `int`
/// (lignes affectées) — auto-détecté via `Statement::num_columns()`. En cas
/// d'échec à n'importe quelle étape, lève `MySQLException` SANS rollback
/// automatique : `rollback()` doit être appelé explicitement — voir
/// `docs/builtins/MySQL.md`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_commit(db_ptr: i64, close: i64) -> i64 {
    unsafe {
        let db = &*(db_ptr as *const OcaraMySQLDatabase);

        let query = match db.pending_query.lock().unwrap().clone() {
            Some(q) => q,
            None => throw_mysql_exception(
                "commit() called without a pending prepare() — call prepare(query) first",
                ERR_COMMIT,
                "MySQL"
            ),
        };
        let binds = db.pending_binds.lock().unwrap().clone().unwrap_or_default();

        // Voir le commentaire de `MySQL_execute` : verrou scopé à la
        // closure, relâché avant tout throw_mysql_exception.
        let result: Result<i64, String> = (|| {
            let mut conn_guard = db.pinned_conn.lock().unwrap();
            let conn = conn_guard.as_mut()
                .ok_or_else(|| "internal error: no pinned connection for commit()".to_string())?;

            conn.query_drop("START TRANSACTION")
                .map_err(|e| format!("Failed to START TRANSACTION: {}", e))?;
            *db.tx_open.lock().unwrap() = true;

            let stmt = conn.prep(&query)
                .map_err(|e| format!("Failed to prepare query '{}': {}", query, e))?;

            let params = params_from_binds(&binds);
            let outcome: Result<i64, String> = if stmt.num_columns() > 0 {
                let result_array = crate::__array_new();
                let query_result = conn.exec_iter(&stmt, params)
                    .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;
                for row_result in query_result {
                    let row = row_result
                        .map_err(|e| format!("Failed to read row for query '{}': {}", query, e))?;
                    crate::__array_push(result_array, row_to_map(&row));
                }
                Ok(result_array)
            } else {
                conn.exec_drop(&stmt, params)
                    .map_err(|e| format!("Failed to execute query '{}': {}", query, e))?;
                let affected = conn.affected_rows() as i64;
                *db.affected_rows.lock().unwrap() = affected;
                *db.last_insert_id.lock().unwrap() = conn.last_insert_id() as i64;
                Ok(crate::box_int_if_needed(affected))
            };
            let value = outcome?;

            conn.query_drop("COMMIT")
                .map_err(|e| format!("Failed to COMMIT transaction: {}", e))?;

            Ok(value)
        })();

        match result {
            Ok(value) => {
                *db.pending_query.lock().unwrap() = None;
                *db.pending_binds.lock().unwrap() = None;
                *db.tx_open.lock().unwrap() = false;
                *db.pinned_conn.lock().unwrap() = None; // rendue au pool
                if close != 0 {
                    MySQL_close(db_ptr);
                }
                value
            }
            Err(msg) => throw_mysql_exception(&msg, ERR_COMMIT, "MySQL"),
        }
    }
}

/// db.rollback() → void — équivalent à rollback(close=false).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_rollback_0(db_ptr: i64) {
    unsafe { MySQL_rollback(db_ptr, 0) }
}

/// db.rollback(close:bool = false) → void
/// Annule la transaction laissée ouverte par un `commit()` échoué. No-op
/// silencieux s'il n'y a rien à annuler — voir `sqlite::SQLite_rollback`
/// pour la justification complète (même patron ici).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MySQL_rollback(db_ptr: i64, close: i64) {
    unsafe {
        let db = &*(db_ptr as *const OcaraMySQLDatabase);
        rollback_pending(db);
        if close != 0 {
            MySQL_close(db_ptr);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// lastInsertId / affectedRows / close
// ─────────────────────────────────────────────────────────────────────────────

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
        // Le pool (et la connexion épinglée éventuelle) sera automatiquement
        // fermé quand la structure est drop.
        let _ = Box::from_raw(db_ptr as *mut OcaraMySQLDatabase);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MariaDB — alias, mêmes fonctions
// ─────────────────────────────────────────────────────────────────────────────

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
pub unsafe extern "C" fn MariaDB_execute_1(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { MySQL_execute_1(db_ptr, query_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_execute_2(db_ptr: i64, query_ptr: i64, placeholder_ptr: i64) -> i64 {
    unsafe { MySQL_execute_2(db_ptr, query_ptr, placeholder_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_execute(db_ptr: i64, query_ptr: i64, placeholder_ptr: i64, close: i64) -> i64 {
    unsafe { MySQL_execute(db_ptr, query_ptr, placeholder_ptr, close) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_query_1(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { MySQL_query_1(db_ptr, query_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_query(db_ptr: i64, query_ptr: i64, placeholder_ptr: i64) -> i64 {
    unsafe { MySQL_query(db_ptr, query_ptr, placeholder_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_queryOne_1(db_ptr: i64, query_ptr: i64) -> i64 {
    unsafe { MySQL_queryOne_1(db_ptr, query_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_queryOne(db_ptr: i64, query_ptr: i64, placeholder_ptr: i64) -> i64 {
    unsafe { MySQL_queryOne(db_ptr, query_ptr, placeholder_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_prepare(db_ptr: i64, query_ptr: i64) {
    unsafe { MySQL_prepare(db_ptr, query_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_bind(db_ptr: i64, placeholders_ptr: i64) {
    unsafe { MySQL_bind(db_ptr, placeholders_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_commit_0(db_ptr: i64) -> i64 {
    unsafe { MySQL_commit_0(db_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_commit(db_ptr: i64, close: i64) -> i64 {
    unsafe { MySQL_commit(db_ptr, close) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_rollback_0(db_ptr: i64) {
    unsafe { MySQL_rollback_0(db_ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn MariaDB_rollback(db_ptr: i64, close: i64) {
    unsafe { MySQL_rollback(db_ptr, close) }
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
