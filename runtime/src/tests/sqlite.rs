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

// ── Groupe 3 : boxing des colonnes INTEGER (SIGSEGV corrigé) ───────────────
// docs/roadmap.d/stdlib-sqlite-integer-column-boxing.md — `collect_all_rows`
// (query()/commit() SELECT) ET `SQLite_queryOne` stockaient la valeur `i64`
// brute d'une colonne INTEGER dans le `mixed` de la map résultat, SANS jamais
// passer par `box_int_if_needed`. Un entier >= PTR_THRESHOLD (0x10000) dont
// les 2 bits bas valent 1/2/3 devient alors indiscernable d'un float/bool/int
// BOXÉ (voir `is_float_box`/`is_bool_box`/`is_int_box`, runtime/src/lib.rs) :
// tout consommateur `mixed` générique (`__mixed_to_int`, appelé par le
// lowering dès qu'un résultat de requête est affecté à un `int` concret) le
// déballe alors comme un pointeur et le DÉRÉFÉRENCE — SIGSEGV. Ces tests
// exercent le vrai chemin FFI (`SQLite_query_1`/`SQLite_queryOne_1` sur une
// base `:memory:`), PAS une réimplémentation séparée : avant le correctif,
// certains d'entre eux faisaient planter le PROCESSUS de test lui-même (pas
// juste échouer une assertion), le signal de régression le plus fort possible
// pour ce genre de bug.
//
// Couvre aussi le point spécifiquement signalé comme jamais testé nulle part
// dans ce projet avant ce ticket : le chemin MULTI-LIGNES de `db.query()`
// (`examples/tests/55_sqlite_real_column_boxingTest.oc` et tous les repros
// ad-hoc précédents n'inséraient jamais plus d'une ligne).

/// Valeur `i64` >= `PTR_THRESHOLD` dont les 2 bits bas sont non-nuls — donc
/// AMBIGUË avec un pointeur boxé si elle n'est jamais passée par
/// `box_int_if_needed` avant d'être logée dans un `mixed`. Un vrai timestamp
/// Unix réaliste (le repro original : une colonne `created_at`).
const DANGEROUS_INT: i64 = 1_790_255_242; // & 3 == 2 → confondu avec is_bool_box

#[test]
fn dangerous_int_constant_is_actually_dangerous() {
    // Précondition du reste de ce groupe : si cette assertion casse un jour
    // (ex. quelqu'un change PTR_THRESHOLD), les tests suivants doivent être
    // reconsidérés, pas silencieusement devenir des faux-négatifs.
    assert!(DANGEROUS_INT >= 0x10000);
    assert_eq!(DANGEROUS_INT & 3, 2, "précondition : confondu avec is_bool_box si jamais boxé");
}

#[test]
fn sqlite_query_boxes_large_integer_column_roundtrips_without_crash() {
    // Repro exact du ticket : une seule ligne, une colonne INTEGER dangereuse
    // parmi d'autres colonnes (dont une REAL) — avant le correctif, ce test
    // faisait planter le processus (SIGSEGV dans __mixed_to_int).
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_execute_1(db, alloc_str(
            "CREATE TABLE maintenances (id INTEGER PRIMARY KEY, car_id INTEGER, type TEXT, description TEXT, cost REAL, created_at INTEGER)"
        ));
        crate::sqlite::SQLite_execute_1(db, alloc_str(&format!(
            "INSERT INTO maintenances (car_id, type, description, cost, created_at) VALUES (1, 'improvement', 'Peinture', 500.0, {})",
            DANGEROUS_INT
        )));

        let rows = crate::sqlite::SQLite_query_1(db, alloc_str("SELECT * FROM maintenances"));
        assert_eq!(__array_len(rows), 1);
        let row = __array_get(rows, 0);
        let created_at = __map_get(row, alloc_str("created_at"));
        // Le consommateur générique réel (affectation vers un `int` concret,
        // voir le lowering) déballe toujours via __mixed_to_int.
        assert_eq!(__mixed_to_int(created_at), DANGEROUS_INT);

        crate::sqlite::SQLite_close(db);
    }
}

#[test]
fn sqlite_query_one_boxes_large_integer_column_roundtrips_without_crash() {
    // Même correctif, chemin `queryOne` (copie séparée avant ce ticket).
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_execute_1(db, alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY, stamp INTEGER)"));
        crate::sqlite::SQLite_execute_1(db, alloc_str(&format!(
            "INSERT INTO t (stamp) VALUES ({})", DANGEROUS_INT
        )));

        let row = crate::sqlite::SQLite_queryOne_1(db, alloc_str("SELECT * FROM t"));
        let stamp = __map_get(row, alloc_str("stamp"));
        assert_eq!(__mixed_to_int(stamp), DANGEROUS_INT);

        crate::sqlite::SQLite_close(db);
    }
}

#[test]
fn sqlite_query_multi_row_with_dangerous_integers_on_every_row() {
    // Le point explicitement signalé comme jamais couvert : `db.query()` avec
    // PLUSIEURS lignes. Chaque ligne porte une valeur dangereuse DIFFÉRENTE,
    // couvrant les 3 tags ambigus (is_float_box/is_bool_box/is_int_box) —
    // vérifie qu'aucune ligne n'aliase/n'écrase le boxing d'une autre (chaque
    // `box_int_if_needed` alloue sa PROPRE cellule heap, voir runtime/src/lib.rs).
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_execute_1(db, alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY, big INTEGER, cost REAL, label TEXT)"));

        let values: [(i64, f64, &str); 5] = [
            (100_000, 1.5, "a"),  // & 3 == 0 (jamais ambigu)
            (100_001, 2.5, "b"),  // & 3 == 1 → is_float_box
            (100_002, 3.5, "c"),  // & 3 == 2 → is_bool_box
            (100_003, 4.5, "d"),  // & 3 == 3 → is_int_box
            (DANGEROUS_INT, 5.5, "e"),
        ];
        for (big, cost, label) in values.iter() {
            crate::sqlite::SQLite_execute_1(db, alloc_str(&format!(
                "INSERT INTO t (big, cost, label) VALUES ({}, {}, '{}')", big, cost, label
            )));
        }

        let rows = crate::sqlite::SQLite_query_1(db, alloc_str("SELECT * FROM t ORDER BY id"));
        assert_eq!(__array_len(rows), 5);
        for (idx, (expected_big, expected_cost, expected_label)) in values.iter().enumerate() {
            let row = __array_get(rows, idx as i64);
            let big = __map_get(row, alloc_str("big"));
            let cost = __map_get(row, alloc_str("cost"));
            let label = __map_get(row, alloc_str("label"));
            assert_eq!(__mixed_to_int(big), *expected_big, "ligne {idx}: colonne big corrompue");
            assert_eq!(__mixed_to_float(cost), *expected_cost, "ligne {idx}: colonne cost corrompue");
            assert_eq!(ptr_to_str(label), *expected_label, "ligne {idx}: colonne label corrompue");
        }

        crate::sqlite::SQLite_close(db);
    }
}

#[test]
fn sqlite_query_negative_large_integer_stays_raw_and_correct() {
    // Un entier négatif n'est jamais ambigu avec un pointeur (voir la doc de
    // `box_int_if_needed`) — reste brut quelle que soit sa magnitude, doit
    // rester exact après un aller-retour SELECT.
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_execute_1(db, alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)"));
        crate::sqlite::SQLite_execute_1(db, alloc_str("INSERT INTO t (n) VALUES (-987654321)"));

        let row = crate::sqlite::SQLite_queryOne_1(db, alloc_str("SELECT * FROM t"));
        let n = __map_get(row, alloc_str("n"));
        assert_eq!(__mixed_to_int(n), -987654321);

        crate::sqlite::SQLite_close(db);
    }
}

#[test]
fn sqlite_query_zero_integer_column_is_not_confused_with_null() {
    // `0` est le second cas spécial de `box_int_if_needed` (voir sa doc) —
    // un entier `0` authentique doit rester distinguable de `null` après un
    // aller-retour SELECT, pas seulement les grandes magnitudes.
    unsafe {
        let db = open_memory_db();
        crate::sqlite::SQLite_execute_1(db, alloc_str("CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER, missing INTEGER)"));
        crate::sqlite::SQLite_execute_1(db, alloc_str("INSERT INTO t (n) VALUES (0)"));

        let row = crate::sqlite::SQLite_queryOne_1(db, alloc_str("SELECT * FROM t"));
        let n = __map_get(row, alloc_str("n"));
        let missing = __map_get(row, alloc_str("missing"));
        assert_ne!(n, 0, "0 authentique ne doit jamais être le pointeur nul (donc jamais confondu avec NULL)");
        assert_eq!(__mixed_to_int(n), 0);
        assert_eq!(missing, 0, "une colonne NULL reste le pointeur nul (0)");

        crate::sqlite::SQLite_close(db);
    }
}
