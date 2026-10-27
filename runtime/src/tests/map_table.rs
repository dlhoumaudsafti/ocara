// Table d'une map (map_table.rs, map_hash.rs) : localité des clés à suffixe
// numérique, suppression sans case tombale, clés entières brutes.

use crate::map_hash::{hash_int, hash_text};
use crate::map_table::{Key, KeyRef, MapTable};
use crate::*;

fn insert(table: &mut MapTable, text: &str, value: i64) -> Option<i64> {
    let shared = unsafe { alloc_str(text) };
    let old = table.insert(Key::Text(text), value, Some(shared));
    unsafe { crate::rc::release_string(shared) };
    old
}

fn get(table: &MapTable, text: &str) -> Option<i64> {
    table.get(Key::Text(text))
}

#[test]
fn suffixed_keys_stay_close_to_their_position() {
    let mut table = MapTable::default();
    for i in 0..200_000 {
        insert(&mut table, &format!("k{}", i), i);
    }
    for i in 0..200_000 {
        assert_eq!(get(&table, &format!("k{}", i)), Some(i));
    }
    assert!(table.mean_displacement() < 1.0, "déplacement moyen {}", table.mean_displacement());
}

#[test]
fn keys_differing_by_padding_or_sign_are_distinct() {
    let mut table = MapTable::default();
    for text in ["k7", "k007", "7", "07", "-7", "-0", "0", "", "-"] {
        assert_eq!(insert(&mut table, text, text.len() as i64), None, "{:?}", text);
    }
    for text in ["k7", "k007", "7", "07", "-7", "-0", "0", "", "-"] {
        assert_eq!(get(&table, text), Some(text.len() as i64), "{:?}", text);
    }
    assert_eq!(insert(&mut table, "k7", 99), Some(2));
    assert_eq!(table.len(), 9);
}

/// Clés à suffixe (`item_N`) et entiers multiples de 1024 (même position
/// modulo la taille de l'index : longues suites de sondage).
#[test]
fn removal_keeps_every_other_key_reachable_and_ordered() {
    for name in [|i: i64| format!("item_{}", i), |i: i64| format!("{}", i * 1024)] {
        let mut table = MapTable::default();
        for i in 0..5000 {
            insert(&mut table, &name(i * 7 % 5000), i);
        }
        for i in (0..5000).filter(|i| i % 3 == 0) {
            assert!(table.remove(Key::Text(&name(i))).is_some());
        }
        for i in 0..5000 {
            assert_eq!(table.contains_key(Key::Text(&name(i))), i % 3 != 0, "{}", name(i));
        }
        let order: Vec<i64> = table.values().collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(table.len(), order.len());
    }
}

#[test]
fn an_int_and_its_decimal_text_hash_alike() {
    let seed = Default::default();
    for value in [0, 1, -1, 9, 10, -10, 4096, 1 << 40, -(1 << 40), 999_999_999_999_999_999, i64::MAX, i64::MIN] {
        assert_eq!(hash_int(value), hash_text(&seed, &value.to_string()), "{}", value);
    }
}

/// Clés entières rangées brutes, retrouvées par leur texte ; une clé non
/// entière fait passer la table aux chaînes sans rien perdre.
#[test]
fn int_keys_switch_to_text_keys_on_the_first_other_key() {
    let mut table = MapTable::default();
    for i in -500..500 {
        assert_eq!(table.insert(Key::Int(i), i * 2, None), None);
    }
    assert_eq!(insert(&mut table, "7", 0), Some(14));
    assert!(!table.has_text_keys());
    assert_eq!(get(&table, "07"), None);
    assert_eq!(get(&table, "-3"), Some(-6));
    assert_eq!(table.remove(Key::Text("-500")), Some(-1000));
    insert(&mut table, "k", 1);
    assert!(table.has_text_keys());
    for i in -499..500 {
        let expected = if i == 7 { 0 } else { i * 2 };
        assert_eq!(table.get(Key::Int(i)), Some(expected), "{}", i);
        assert_eq!(get(&table, &i.to_string()), Some(expected), "{}", i);
    }
    let keys: Vec<String> = table.keys().map(|k| k.with_text(str::to_string)).collect();
    assert_eq!(keys.first().map(String::as_str), Some("-499"));
    assert_eq!(keys.last().map(String::as_str), Some("k"));
}

#[test]
fn value_at_reads_the_pair_at_its_rank_or_searches() {
    let mut table = MapTable::default();
    for i in 0..10 {
        table.insert(Key::Int(i), i + 100, None);
    }
    assert_eq!(table.value_at(3, Key::Int(3)), Some(103));
    assert_eq!(table.value_at(3, Key::Int(8)), Some(108));
    assert_eq!(table.value_at(42, Key::Text("9")), Some(109));
    assert_eq!(table.value_at(0, Key::Int(10)), None);
    assert!(matches!(table.keys().next(), Some(KeyRef::Int(0))));
}

/// Petite table (sans index) : mêmes résultats avant et après la
/// construction de l'index à la neuvième clé, suppressions comprises.
#[test]
fn small_tables_switch_to_an_index_without_losing_keys() {
    let mut table = MapTable::default();
    for i in 0..8 {
        assert_eq!(insert(&mut table, &format!("key{}", i), i), None);
    }
    assert_eq!(table.mean_displacement(), 0.0, "pas encore d'index");
    assert_eq!(insert(&mut table, "key3", 30), Some(3));
    assert_eq!(table.remove(Key::Text("key0")), Some(0));
    assert_eq!(get(&table, "key0"), None);
    for i in 8..20 {
        insert(&mut table, &format!("key{}", i), i);
    }
    assert_eq!(table.remove(Key::Text("key5")), Some(5));
    for i in 1..20 {
        let expected = match i { 3 => Some(30), 5 => None, _ => Some(i) };
        assert_eq!(get(&table, &format!("key{}", i)), expected, "key{}", i);
    }
    let order: Vec<String> = table.keys().map(|k| k.with_text(str::to_string)).collect();
    assert_eq!(order.first().map(String::as_str), Some("key1"));
    assert_eq!(order.len(), 18);
}

#[test]
fn small_int_tables_switch_to_text_keys() {
    let mut table = MapTable::default();
    for i in 0..5 {
        table.insert(Key::Int(i * 70_000), i, None);
    }
    assert_eq!(get(&table, "140000"), Some(2));
    insert(&mut table, "x", 9);
    assert!(table.has_text_keys());
    for i in 0..5 {
        assert_eq!(table.get(Key::Int(i * 70_000)), Some(i));
    }
    for i in 0..10 {
        table.insert(Key::Int(-i), -i, None);
    }
    assert_eq!(get(&table, "-9"), Some(-9));
    assert_eq!(get(&table, "x"), Some(9));
}

/// Petite table : une clé cherchée avec la chaîne même qui a été rangée
/// (littéral interné) est trouvée par pointeur ; une copie du même texte,
/// par comparaison du texte.
#[test]
fn small_table_finds_a_stored_key_by_pointer_or_by_text() {
    let mut table = MapTable::default();
    let stored: Vec<i64> = ["alpha", "beta", "gamma", "theta"].iter().map(|t| unsafe { alloc_str(t) }).collect();
    for (i, key) in stored.iter().enumerate() {
        table.insert(Key::Text(unsafe { ptr_to_str(*key) }), i as i64, Some(*key));
    }
    for key in &stored {
        unsafe { crate::rc::release_string(*key) };
    }
    assert_eq!(table.get(Key::Text(unsafe { ptr_to_str(stored[3]) })), Some(3));
    assert_eq!(get(&table, &String::from("theta")), Some(3));
    assert_eq!(get(&table, "delta"), None);
}
