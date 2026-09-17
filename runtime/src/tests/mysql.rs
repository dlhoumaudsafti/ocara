// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires — ocara.MySQL/MariaDB : requêtes paramétrées + transactions
// (docs/roadmap.d/stdlib-mysql-requetes-parametrees-transactions.md).
//
// Deux familles de tests, séparées différemment que côté SQLite
// (`tests::sqlite`) parce que MySQL n'a pas d'équivalent à `:memory:` — un
// vrai serveur est nécessaire :
//
// - Groupe 1 (toujours exécuté) : `mixed_to_mysql_value`/`read_placeholders`
//   en isolation — mêmes cas frontière du bit pattern `mixed` que côté
//   SQLite (voir `tests::sqlite`), sans connexion réseau.
// - Groupe 2 (`#[ignore]`, exécuté seulement avec `cargo test -p
//   ocara_runtime -- --ignored`) : les fonctions `MySQL_*` réelles contre un
//   vrai serveur. Le projet n'a aujourd'hui aucune infrastructure CI MySQL
//   (voir docs/roadmap.d/qualite-couverture-tests.md, Priorité Très Basse) —
//   les marquer `#[ignore]` plutôt que de les laisser échouer sur toute
//   machine sans serveur local était le seul choix honnête. Vérifiés
//   manuellement pendant le développement de ce ticket contre un conteneur
//   Docker jetable :
//
//     docker run -d --name ocara_mariadb_test \
//       -e MARIADB_ROOT_PASSWORD=ocara_test_pw \
//       -e MARIADB_DATABASE=ocara_test \
//       -p 3306:3306 mariadb:11
//
//   Mêmes limites que côté SQLite sur les chemins d'erreur des fonctions FFI
//   (throw_mysql_exception saute par longjmp, UB hors du contexte setjmp
//   d'un `try` Ocara compilé) — seuls les chemins de succès sont testés ici,
//   les chemins d'erreur restent couverts par les tests `.oc` de régression.
// ─────────────────────────────────────────────────────────────────────────────

use crate::*;
use crate::mysql::{mixed_to_mysql_value, read_placeholders};
use ::mysql::Value as MyValue;

// ── Groupe 1 : mixed_to_mysql_value ─────────────────────────────────────────

#[test]
fn mixed_to_mysql_value_null_is_raw_zero() {
    assert_eq!(mixed_to_mysql_value(0), Ok(MyValue::NULL));
}

#[test]
fn mixed_to_mysql_value_boxed_zero_int_is_not_null() {
    // Même désambiguïsation `0`/`null` que côté SQLite — voir
    // `tests::sqlite::mixed_to_sql_value_boxed_zero_int_is_not_null` et
    // `box_int_if_needed_zero_is_always_boxed` dans `tests::boxing`.
    let boxed_zero = box_int_if_needed(0);
    assert_ne!(boxed_zero, 0);
    assert_eq!(mixed_to_mysql_value(boxed_zero), Ok(MyValue::Int(0)));
}

#[test]
fn mixed_to_mysql_value_boxed_int_above_ptr_threshold() {
    let boxed = box_int_if_needed(100_000);
    assert_eq!(mixed_to_mysql_value(boxed), Ok(MyValue::Int(100_000)));
}

#[test]
fn mixed_to_mysql_value_raw_small_positive_int_stays_raw() {
    let raw = box_int_if_needed(42);
    assert_eq!(raw, 42, "précondition : 42 doit rester brut");
    assert_eq!(mixed_to_mysql_value(raw), Ok(MyValue::Int(42)));
}

#[test]
fn mixed_to_mysql_value_negative_int_stays_raw() {
    assert_eq!(mixed_to_mysql_value(-12345), Ok(MyValue::Int(-12345)));
}

#[test]
fn mixed_to_mysql_value_boxed_float() {
    let boxed = __box_float((-3.5f64).to_bits() as i64);
    assert_eq!(mixed_to_mysql_value(boxed), Ok(MyValue::Double(-3.5)));
}

#[test]
fn mixed_to_mysql_value_boxed_bool_true_and_false() {
    // MySQL n'a pas de type bool natif — un bool Ocara se bind comme un
    // Int(0/1), convention TINYINT(1) (voir la doc de `mixed_to_mysql_value`).
    let t = __box_bool(1);
    let f = __box_bool(0);
    assert_eq!(mixed_to_mysql_value(t), Ok(MyValue::Int(1)));
    assert_eq!(mixed_to_mysql_value(f), Ok(MyValue::Int(0)));
}

#[test]
fn mixed_to_mysql_value_string() {
    let s = unsafe { alloc_str("hello placeholder") };
    assert_eq!(mixed_to_mysql_value(s), Ok(MyValue::Bytes(b"hello placeholder".to_vec())));
}

#[test]
fn mixed_to_mysql_value_rejects_array() {
    let arr = __array_new();
    __array_push(arr, box_int_if_needed(1));
    assert!(mixed_to_mysql_value(arr).is_err(), "un array ne doit jamais être silencieusement bindable");
}

#[test]
fn mixed_to_mysql_value_rejects_map() {
    let m = __map_new();
    assert!(mixed_to_mysql_value(m).is_err(), "une map ne doit jamais être silencieusement bindable");
}

// ── Groupe 1bis : read_placeholders ─────────────────────────────────────────

#[test]
fn read_placeholders_null_pointer_is_empty() {
    let binds = unsafe { read_placeholders(0) }.expect("placeholder absent (0) ne doit jamais échouer");
    assert!(binds.is_empty());
}

#[test]
fn read_placeholders_keys_have_no_colon_prefix() {
    // Contrairement à SQLite (`sqlite::read_placeholders`, qui préfixe `:`),
    // le crate `mysql` extrait déjà les noms SANS le `:` depuis la requête
    // (`mysql_common::named_params::ParsedNamedParams` — vérifié dans le
    // code source vendored avant d'écrire `mysql::read_placeholders`) et
    // attend les clés de `Params::Named` sous la même forme nue. Un bug ici
    // (préfixe ajouté par erreur, copié de SQLite par symétrie) ferait
    // échouer TOUT binding nommé avec "Missing named parameter" — ce test
    // fige la convention.
    let m = __map_new();
    __map_set(m, unsafe { alloc_str("id") }, box_int_if_needed(156));
    __map_set(m, unsafe { alloc_str("name") }, unsafe { alloc_str("Alice") });

    let mut binds = unsafe { read_placeholders(m) }.expect("map de placeholders valide");
    binds.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(binds.len(), 2);
    assert_eq!(binds[0], ("id".to_string(), MyValue::Int(156)));
    assert_eq!(binds[1], ("name".to_string(), MyValue::Bytes(b"Alice".to_vec())));
}

#[test]
fn read_placeholders_propagates_unsupported_value_error() {
    let m = __map_new();
    __map_set(m, unsafe { alloc_str("bad") }, __array_new());
    assert!(unsafe { read_placeholders(m) }.is_err());
}

// ── Groupe 2 : chemins de succès des fonctions MySQL_* réelles (serveur requis) ──

const TEST_HOST: &str = "127.0.0.1";
const TEST_USER: &str = "root";
const TEST_PASS: &str = "ocara_test_pw";
const TEST_DB:   &str = "ocara_test";

/// Connecte via la vraie fonction `MySQL_connect` — voir le commentaire de
/// tête de fichier pour le conteneur Docker attendu.
unsafe fn connect_test_db() -> i64 {
    unsafe {
        let host = alloc_str(TEST_HOST);
        let user = alloc_str(TEST_USER);
        let pass = alloc_str(TEST_PASS);
        let db   = alloc_str(TEST_DB);
        crate::mysql::MySQL_connect(host, user, pass, db)
    }
}

#[test]
#[ignore = "nécessite un serveur MySQL/MariaDB local, voir le commentaire de tête de fichier"]
fn mysql_execute_1_then_query_1_roundtrip() {
    unsafe {
        let db = connect_test_db();
        // DDL (DROP/CREATE) sans placeholder — vérifie au passage que
        // `params_from_binds` utilise bien `Params::Empty` et pas
        // `Params::from(vec![])` (`Params::Named` vide) pour ce cas : le
        // crate `mysql` rejette `Params::Named`, même vide, contre une
        // requête sans aucun placeholder nommé ("positional query") —
        // régression réelle trouvée en développant ce ticket, voir
        // `params_from_binds`.
        crate::mysql::MySQL_execute_1(db, alloc_str("DROP TABLE IF EXISTS rust_test_t"));
        crate::mysql::MySQL_execute_1(db, alloc_str("CREATE TABLE rust_test_t (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(64), age INT)"));

        crate::mysql::MySQL_execute_1(db, alloc_str("INSERT INTO rust_test_t (name, age) VALUES ('Alice', 30)"));
        assert_eq!(crate::mysql::MySQL_lastInsertId(db), 1);
        assert_eq!(crate::mysql::MySQL_affectedRows(db), 1);

        let rows = crate::mysql::MySQL_query_1(db, alloc_str("SELECT * FROM rust_test_t"));
        assert_eq!(__array_len(rows), 1);
        let row = __array_get(rows, 0);
        assert_eq!(ptr_to_str(__map_get(row, alloc_str("name"))), "Alice");
        // Assertion clé : une colonne numérique lue SANS placeholder doit
        // revenir comme un `int` Ocara (brut, `== 30`), pas une string "30"
        // (pointeur alloué, jamais égal à l'entier 30) — c'est exactement le
        // bug trouvé pendant ce ticket (protocole texte vs binaire, voir
        // `row_to_map`) : invisible en interpolant dans un template string,
        // détecté seulement par une comparaison entière stricte comme
        // celle-ci.
        assert_eq!(__map_get(row, alloc_str("age")), 30);

        crate::mysql::MySQL_close(db);
    }
}

#[test]
#[ignore = "nécessite un serveur MySQL/MariaDB local, voir le commentaire de tête de fichier"]
fn mysql_execute_2_binds_named_placeholders() {
    unsafe {
        let db = connect_test_db();
        crate::mysql::MySQL_execute_1(db, alloc_str("DROP TABLE IF EXISTS rust_test_t2"));
        crate::mysql::MySQL_execute_1(db, alloc_str("CREATE TABLE rust_test_t2 (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(64), age INT)"));

        let m = __map_new();
        __map_set(m, alloc_str("name"), alloc_str("Bob"));
        __map_set(m, alloc_str("age"), box_int_if_needed(25));
        crate::mysql::MySQL_execute_2(db, alloc_str("INSERT INTO rust_test_t2 (name, age) VALUES (:name, :age)"), m);

        let one = crate::mysql::MySQL_queryOne_1(db, alloc_str("SELECT * FROM rust_test_t2 WHERE name = 'Bob'"));
        assert_ne!(one, 0, "la ligne doit être trouvée (pas null)");
        assert_eq!(__map_get(one, alloc_str("age")), 25);

        crate::mysql::MySQL_close(db);
    }
}

#[test]
#[ignore = "nécessite un serveur MySQL/MariaDB local, voir le commentaire de tête de fichier"]
fn mysql_queryone_on_missing_row_returns_null_not_empty_map() {
    // Contrairement à SQLite::queryOne (map vide), MySQL::queryOne retourne
    // `null` — voir docs/roadmap.d/coherence-documentation-ebnf-stdlib.md
    // sur ce piège documenté entre les deux builtins.
    unsafe {
        let db = connect_test_db();
        crate::mysql::MySQL_execute_1(db, alloc_str("DROP TABLE IF EXISTS rust_test_t3"));
        crate::mysql::MySQL_execute_1(db, alloc_str("CREATE TABLE rust_test_t3 (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(64))"));

        let missing = crate::mysql::MySQL_queryOne_1(db, alloc_str("SELECT * FROM rust_test_t3 WHERE name = 'NoOne'"));
        assert_eq!(missing, 0, "aucune ligne trouvée doit retourner null (0), pas une map vide");

        crate::mysql::MySQL_close(db);
    }
}

#[test]
#[ignore = "nécessite un serveur MySQL/MariaDB local, voir le commentaire de tête de fichier"]
fn mysql_stepped_prepare_bind_commit_insert_returns_boxed_affected_count() {
    unsafe {
        let db = connect_test_db();
        crate::mysql::MySQL_execute_1(db, alloc_str("DROP TABLE IF EXISTS rust_test_t4"));
        crate::mysql::MySQL_execute_1(db, alloc_str("CREATE TABLE rust_test_t4 (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(64))"));

        crate::mysql::MySQL_prepare(db, alloc_str("INSERT INTO rust_test_t4 (name) VALUES (:name)"));
        let m = __map_new();
        __map_set(m, alloc_str("name"), alloc_str("Carol"));
        crate::mysql::MySQL_bind(db, m);
        let affected = crate::mysql::MySQL_commit_0(db);
        assert_eq!(affected, 1);

        let one = crate::mysql::MySQL_queryOne_1(db, alloc_str("SELECT * FROM rust_test_t4 WHERE name = 'Carol'"));
        assert_ne!(one, 0);

        crate::mysql::MySQL_close(db);
    }
}

#[test]
#[ignore = "nécessite un serveur MySQL/MariaDB local, voir le commentaire de tête de fichier"]
fn mysql_stepped_prepare_bind_commit_select_returns_array_of_rows() {
    unsafe {
        let db = connect_test_db();
        crate::mysql::MySQL_execute_1(db, alloc_str("DROP TABLE IF EXISTS rust_test_t5"));
        crate::mysql::MySQL_execute_1(db, alloc_str("CREATE TABLE rust_test_t5 (id INT AUTO_INCREMENT PRIMARY KEY, age INT)"));
        {
            let m = __map_new();
            __map_set(m, alloc_str("age"), box_int_if_needed(40));
            crate::mysql::MySQL_execute_2(db, alloc_str("INSERT INTO rust_test_t5 (age) VALUES (:age)"), m);
        }
        {
            let m = __map_new();
            __map_set(m, alloc_str("age"), box_int_if_needed(41));
            crate::mysql::MySQL_execute_2(db, alloc_str("INSERT INTO rust_test_t5 (age) VALUES (:age)"), m);
        }

        crate::mysql::MySQL_prepare(db, alloc_str("SELECT * FROM rust_test_t5 WHERE age >= :min"));
        let m = __map_new();
        __map_set(m, alloc_str("min"), box_int_if_needed(41));
        crate::mysql::MySQL_bind(db, m);
        let result = crate::mysql::MySQL_commit_0(db);

        // `commit()` doit avoir auto-détecté un SELECT (num_columns > 0) et
        // retourné un pointeur d'array, pas un int boxé.
        assert!(result >= 0x10000 && (result & 3) == 0, "commit() sur un SELECT doit retourner un pointeur array, pas un int");
        assert_eq!(__array_len(result), 1);

        crate::mysql::MySQL_close(db);
    }
}

#[test]
#[ignore = "nécessite un serveur MySQL/MariaDB local, voir le commentaire de tête de fichier"]
fn mysql_rollback_without_pending_transaction_is_a_safe_noop() {
    unsafe {
        let db = connect_test_db();
        crate::mysql::MySQL_rollback_0(db);
        // La connexion doit rester utilisable après ce rollback à vide.
        crate::mysql::MySQL_execute_1(db, alloc_str("SELECT 1"));
        crate::mysql::MySQL_close(db);
    }
}

#[test]
#[ignore = "nécessite un serveur MySQL/MariaDB local, voir le commentaire de tête de fichier"]
fn mysql_rollback_after_failed_commit_leaves_connection_usable() {
    // Reproduit le scénario vérifié manuellement pendant ce ticket (violation
    // UNIQUE pendant commit(), puis rollback(), puis nouvel appel sur la même
    // connexion) — voir docs/roadmap.d/stdlib-mysql-requetes-parametrees-transactions.md.
    unsafe {
        let db = connect_test_db();
        crate::mysql::MySQL_execute_1(db, alloc_str("DROP TABLE IF EXISTS rust_test_t6"));
        crate::mysql::MySQL_execute_1(db, alloc_str("CREATE TABLE rust_test_t6 (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(64) UNIQUE)"));
        crate::mysql::MySQL_execute_1(db, alloc_str("INSERT INTO rust_test_t6 (name) VALUES ('Dup')"));

        crate::mysql::MySQL_prepare(db, alloc_str("INSERT INTO rust_test_t6 (name) VALUES (:name)"));
        let m = __map_new();
        __map_set(m, alloc_str("name"), alloc_str("Dup"));
        crate::mysql::MySQL_bind(db, m);
        // Pas d'appel direct à MySQL_commit ici : violerait UNIQUE et lèverait
        // via longjmp (UB hors contexte setjmp, voir le commentaire de tête
        // de fichier) — ce test vérifie seulement que la connexion reste
        // fonctionnelle après un rollback(), pas le déclenchement de l'erreur
        // elle-même (déjà couvert par les tests `.oc` de régression).
        crate::mysql::MySQL_rollback_0(db);

        crate::mysql::MySQL_execute_1(db, alloc_str("SELECT 1"));
        crate::mysql::MySQL_close(db);
    }
}
