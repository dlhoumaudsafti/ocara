// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires — ocara.SQLite : requêtes paramétrées + transactions
// (docs/roadmap.d/stdlib-sqlite-requetes-parametrees-transactions.md).
//
// Deux familles de tests :
// - Groupe 1 : `mixed_to_sql_value`/`read_placeholders` en isolation — les
//   fonctions pures de conversion, sans FFI. Couvre exhaustivement les cas
//   frontière du bit pattern `mixed` (voir `tests::boxing`) tels qu'ils
//   affectent spécifiquement le bind SQL : `0` (null vs int boxé), boxé vs
//   brut, type non bindable.
// - Groupe 2 : les fonctions `SQLite_*` exportées elles-mêmes, sur base
//   `:memory:` — le chemin RÉEL emprunté par un programme Ocara compilé,
//   pas une réimplémentation séparée de la même logique.
//
// Limite assumée : seuls les chemins de SUCCÈS des fonctions FFI sont
// testés ici. `throw_sqlite_exception` (via `__ocara_fail`) saute par
// `longjmp` vers un `jmp_buf` établi par le code généré pour un `try` Ocara
// — l'invoquer depuis un `#[test]` Rust ordinaire, hors de ce contexte,
// serait un comportement indéfini (pas de `jmp_buf` à sauter). Les chemins
// d'erreur (prepare() invalide, bind() sans prepare(), échec de commit())
// restent couverts par les tests de régression `.oc` (compilés + exécutés
// avec le vrai mécanisme setjmp/longjmp en place), pas ici.
// ─────────────────────────────────────────────────────────────────────────────

use crate::*;
use crate::sqlite::{mixed_to_sql_value, read_placeholders};
use rusqlite::types::Value as SqlValue;

// ── Groupe 1 : mixed_to_sql_value ───────────────────────────────────────────

#[test]
fn mixed_to_sql_value_null_is_raw_zero() {
    assert_eq!(mixed_to_sql_value(0), Ok(SqlValue::Null));
}

#[test]
fn mixed_to_sql_value_boxed_zero_int_is_not_null() {
    // Coeur de la désambiguïsation `0`/`null` (voir `box_int_if_needed`,
    // `tests::boxing::box_int_if_needed_zero_is_always_boxed`) : un `int`
    // valant `0` logé dans `map<string,mixed>` doit binder `0`, pas `NULL`.
    let boxed_zero = box_int_if_needed(0);
    assert_ne!(boxed_zero, 0, "0 boxé ne doit jamais être le pointeur nul");
    assert_eq!(mixed_to_sql_value(boxed_zero), Ok(SqlValue::Integer(0)));
}

#[test]
fn mixed_to_sql_value_boxed_int_above_ptr_threshold() {
    let boxed = box_int_if_needed(100_000);
    assert_eq!(mixed_to_sql_value(boxed), Ok(SqlValue::Integer(100_000)));
}

#[test]
fn mixed_to_sql_value_raw_small_positive_int_stays_raw() {
    // 42 est trop petit pour être confondu avec un pointeur heap — jamais
    // boxé par `box_int_if_needed`, donc lu directement par `get_value_type`
    // (branche "primitif").
    let raw = box_int_if_needed(42);
    assert_eq!(raw, 42, "42 doit rester brut (précondition du test)");
    assert_eq!(mixed_to_sql_value(raw), Ok(SqlValue::Integer(42)));
}

#[test]
fn mixed_to_sql_value_negative_int_stays_raw() {
    // Un entier négatif n'est jamais ambigu avec un pointeur (toujours
    // < PTR_THRESHOLD en comparaison signée) — reste brut quelle que soit
    // sa magnitude, voir la doc de `box_int_if_needed`.
    assert_eq!(mixed_to_sql_value(-12345), Ok(SqlValue::Integer(-12345)));
}

#[test]
fn mixed_to_sql_value_boxed_float() {
    let boxed = __box_float((-3.5f64).to_bits() as i64);
    assert_eq!(mixed_to_sql_value(boxed), Ok(SqlValue::Real(-3.5)));
}

#[test]
fn mixed_to_sql_value_boxed_bool_true_and_false() {
    let t = __box_bool(1);
    let f = __box_bool(0);
    assert_eq!(mixed_to_sql_value(t), Ok(SqlValue::Integer(1)));
    assert_eq!(mixed_to_sql_value(f), Ok(SqlValue::Integer(0)));
}

#[test]
fn mixed_to_sql_value_string() {
    let s = unsafe { alloc_str("hello placeholder") };
    assert_eq!(mixed_to_sql_value(s), Ok(SqlValue::Text("hello placeholder".to_string())));
}

#[test]
fn mixed_to_sql_value_rejects_array() {
    let arr = __array_new();
    __array_push(arr, box_int_if_needed(1));
    let result = mixed_to_sql_value(arr);
    assert!(result.is_err(), "un array ne doit jamais être silencieusement bindable");
}

#[test]
fn mixed_to_sql_value_rejects_map() {
    let m = __map_new();
    let result = mixed_to_sql_value(m);
    assert!(result.is_err(), "une map ne doit jamais être silencieusement bindable");
}

// ── Groupe 1bis : read_placeholders ─────────────────────────────────────────

#[test]
fn read_placeholders_null_pointer_is_empty() {
    let binds = unsafe { read_placeholders(0) }.expect("placeholder absent (0) ne doit jamais échouer");
    assert!(binds.is_empty());
}

#[test]
fn read_placeholders_adds_colon_prefix_and_converts_values() {
    // bind({"id": 156, "name": "Alice"}) — la map Ocara porte des clés NUES
    // ("id"), la requête SQL écrit `:id` : c'est `read_placeholders` qui
    // fait le pont entre les deux conventions (voir sa doc dans sqlite.rs).
    let m = __map_new();
    __map_set(m, unsafe { alloc_str("id") }, box_int_if_needed(156));
    __map_set(m, unsafe { alloc_str("name") }, unsafe { alloc_str("Alice") });

    let mut binds = unsafe { read_placeholders(m) }.expect("map de placeholders valide");
    binds.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(binds.len(), 2);
    assert_eq!(binds[0], (":id".to_string(), SqlValue::Integer(156)));
    assert_eq!(binds[1], (":name".to_string(), SqlValue::Text("Alice".to_string())));
}

#[test]
fn read_placeholders_propagates_unsupported_value_error() {
    let m = __map_new();
    __map_set(m, unsafe { alloc_str("bad") }, __array_new());
    let result = unsafe { read_placeholders(m) };
    assert!(result.is_err(), "une valeur non bindable dans la map doit remonter une erreur, pas planter/ignorer");
}

// ── Groupe 2 : chemins de succès des fonctions SQLite_* réelles ────────────

/// Ouvre une base `:memory:` fraîche via la vraie fonction `SQLite_open` —
/// exerce le même chemin qu'un programme Ocara compilé, pas une
/// réimplémentation de test.
unsafe fn open_memory_db() -> i64 {
    let path = unsafe { alloc_str(":memory:") };
    unsafe { crate::sqlite::SQLite_open(path) }
}

#[test]
fn sqlite_execute_1_then_query_1_roundtrip() {
    unsafe {
        let db = open_memory_db();
        let create = alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)");
        crate::sqlite::SQLite_execute_1(db, create);

        let insert = alloc_str("INSERT INTO t (name) VALUES ('Alice')");
        crate::sqlite::SQLite_execute_1(db, insert);
        assert_eq!(crate::sqlite::SQLite_lastInsertId(db), 1);
        assert_eq!(crate::sqlite::SQLite_affectedRows(db), 1);

        let select = alloc_str("SELECT * FROM t");
        let rows = crate::sqlite::SQLite_query_1(db, select);
        assert_eq!(__array_len(rows), 1);
        let row = __array_get(rows, 0);
        let name_key = alloc_str("name");
        assert_eq!(ptr_to_str(__map_get(row, name_key)), "Alice");

        crate::sqlite::SQLite_close(db);
    }
}

#[test]
fn sqlite_execute_2_binds_named_placeholders() {
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_execute_1(db, alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)"));

        let m = __map_new();
        __map_set(m, alloc_str("name"), alloc_str("Bob"));
        __map_set(m, alloc_str("age"), box_int_if_needed(25));
        crate::sqlite::SQLite_execute_2(db, alloc_str("INSERT INTO t (name, age) VALUES (:name, :age)"), m);

        let one = crate::sqlite::SQLite_queryOne_1(db, alloc_str("SELECT * FROM t WHERE name = 'Bob'"));
        let age_key = alloc_str("age");
        let age_val = __map_get(one, age_key);
        // Une valeur lue depuis une ligne SQLite (via row.get::<_, i64>) est
        // reboxée par le même chemin que query()/queryOne() existants — un
        // int non nul, non ambigu avec un pointeur, reste ici un i64 brut.
        assert_eq!(age_val, 25);

        crate::sqlite::SQLite_close(db);
    }
}

#[test]
fn sqlite_stepped_prepare_bind_commit_insert_returns_boxed_affected_count() {
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_execute_1(db, alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)"));

        crate::sqlite::SQLite_prepare(db, alloc_str("INSERT INTO t (name) VALUES (:name)"));
        let m = __map_new();
        __map_set(m, alloc_str("name"), alloc_str("Carol"));
        crate::sqlite::SQLite_bind(db, m);
        let affected = crate::sqlite::SQLite_commit_0(db);

        // commit() retourne `mixed` : pour une écriture, un `int` boxé
        // (voir box_int_if_needed) — 1 est trop petit pour être ambigu avec
        // un pointeur, mais `commit()` boxe systématiquement via
        // `box_int_if_needed`, donc le round-trip doit rester cohérent avec
        // lui, pas juste "== 1" en valeur brute.
        assert_eq!(box_int_if_needed(1), 1, "précondition : 1 reste brut");
        assert_eq!(affected, 1);

        let one = crate::sqlite::SQLite_queryOne_1(db, alloc_str("SELECT * FROM t WHERE name = 'Carol'"));
        assert_eq!(ptr_to_str(__map_get(one, alloc_str("name"))), "Carol");

        crate::sqlite::SQLite_close(db);
    }
}

#[test]
fn sqlite_stepped_prepare_bind_commit_select_returns_array_of_rows() {
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_execute_1(db, alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY, age INTEGER)"));
        crate::sqlite::SQLite_execute_2(
            db,
            alloc_str("INSERT INTO t (age) VALUES (:age)"),
            { let m = __map_new(); __map_set(m, alloc_str("age"), box_int_if_needed(40)); m },
        );
        crate::sqlite::SQLite_execute_2(
            db,
            alloc_str("INSERT INTO t (age) VALUES (:age)"),
            { let m = __map_new(); __map_set(m, alloc_str("age"), box_int_if_needed(41)); m },
        );

        crate::sqlite::SQLite_prepare(db, alloc_str("SELECT * FROM t WHERE age >= :min"));
        let m = __map_new();
        __map_set(m, alloc_str("min"), box_int_if_needed(41));
        crate::sqlite::SQLite_bind(db, m);
        let result = crate::sqlite::SQLite_commit_0(db);

        // `commit()` doit avoir auto-détecté un SELECT (column_count > 0) et
        // retourné un pointeur d'array, pas un int boxé — is_ptr distingue
        // les deux (voir runtime/src/lib.rs, schéma de tag bits bas 00).
        assert!(is_ptr_for_tests(result), "commit() sur un SELECT doit retourner un pointeur array, pas un int");
        assert_eq!(__array_len(result), 1);

        crate::sqlite::SQLite_close(db);
    }
}

#[test]
fn sqlite_rollback_without_pending_transaction_is_a_safe_noop() {
    // rollback() appelé "à froid" (aucun prepare()/commit() avant) ne doit
    // ni paniquer ni corrompre l'état — c'est le patron d'usage documenté
    // (on l'appelle depuis un `on e is SQLiteException` sans savoir
    // précisément à quelle étape l'échec a eu lieu).
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_rollback_0(db);
        // La connexion doit rester utilisable après ce rollback à vide.
        crate::sqlite::SQLite_execute_1(db, alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY)"));
        crate::sqlite::SQLite_close(db);
    }
}

#[test]
fn sqlite_prepare_resets_pending_binds_from_previous_cycle() {
    // Un `prepare()` doit effacer les binds d'un cycle prepare/bind/commit
    // PRÉCÉDENT — sinon un bind() oublié après un second prepare()
    // réutiliserait silencieusement les valeurs de la requête d'avant.
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_execute_1(db, alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)"));

        crate::sqlite::SQLite_prepare(db, alloc_str("INSERT INTO t (name) VALUES (:name)"));
        let m1 = __map_new();
        __map_set(m1, alloc_str("name"), alloc_str("First"));
        crate::sqlite::SQLite_bind(db, m1);
        crate::sqlite::SQLite_commit_0(db);

        // Deuxième cycle, requête SANS placeholder cette fois — si les binds
        // du premier cycle avaient fui, ils seraient simplement ignorés par
        // `rusqlite` ici (la requête n'a pas de `:name`), donc ce test isolé
        // ne suffirait pas à prouver la fuite — la vraie preuve est le champ
        // `pending_binds` remis à `None` par `prepare()` (voir sqlite.rs) ;
        // ce test vérifie au moins que le flux reste correct dans l'usage
        // normal (pas de crash, pas de valeur résiduelle inattendue).
        crate::sqlite::SQLite_prepare(db, alloc_str("SELECT COUNT(*) as n FROM t"));
        let result = crate::sqlite::SQLite_commit_0(db);
        assert!(is_ptr_for_tests(result));
        let row = __array_get(result, 0);
        assert_eq!(__map_get(row, alloc_str("n")), 1);

        crate::sqlite::SQLite_close(db);
    }
}

/// Petit alias local — `is_ptr` (lib.rs) reste volontairement privé (utilisé
/// uniquement par le schéma de tag interne), ce test n'a besoin que de
/// distinguer "pointeur" de "int boxé/brut" pour vérifier l'auto-détection
/// de `commit()`, donc une copie triviale de la même condition suffit sans
/// élargir la visibilité d'une fonction par ailleurs volontairement interne.
fn is_ptr_for_tests(val: i64) -> bool {
    val >= 0x10000 && (val & 3) == 0
}
