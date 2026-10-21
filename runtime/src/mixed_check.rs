//! Valeur `mixed` rangée dans un type concret (`var u:U = arr[1]` où `arr`
//! est un `array<mixed>`) : `Convert::mixedAs(v, "type")`, inséré par le
//! compilateur, vérifie à l'exécution que la valeur est bien de ce type et
//! lève une `ConvertException` (103) sinon — au lieu d'un SIGSEGV ou d'une
//! adresse lue comme un entier.
//!
//! Classes : `main` enregistre au démarrage (`__class_table_init`) la table
//! `id=Classe|Parent|Interface…;…` ; une instance convient si la classe
//! attendue figure dans la chaîne de sa classe.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::typecheck::{__is_array, __is_function, __is_map, __is_object, __is_string};

static CLASS_TABLE: OnceLock<HashMap<i64, Vec<String>>> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "C" fn __class_table_init(table: i64) {
    let text = if table == 0 { "" } else { unsafe { crate::ptr_to_str(table) } };
    let parsed = text.split(';')
        .filter_map(|entry| entry.split_once('='))
        .filter_map(|(id, chain)| Some((id.parse().ok()?, chain.split('|').map(str::to_string).collect())))
        .collect();
    let _ = CLASS_TABLE.set(parsed);
}

fn class_chain(val: i64) -> Option<&'static Vec<String>> {
    let id = unsafe { *((val - 16) as *const i64) };
    CLASS_TABLE.get()?.get(&id)
}

/// Nom du type de `val`, pour le message d'erreur.
fn found_name(val: i64) -> String {
    if val == 0 {
        return "null".into();
    }
    if crate::is_float_box(val) { return "float".into() }
    if crate::is_bool_box(val) { return "bool".into() }
    if crate::is_int_box(val) || val < 0x10000 || val & 3 != 0 { return "int".into() }
    if __is_string(val) != 0 { return "string".into() }
    if __is_array(val) != 0 { return "array".into() }
    if __is_map(val) != 0 { return "map".into() }
    if __is_function(val) != 0 { return "Function".into() }
    if __is_object(val) != 0 {
        return class_chain(val).and_then(|c| c.first().cloned()).unwrap_or_else(|| "object".into());
    }
    "unknown".into()
}

/// Entier logé dans un `mixed` : brut sous le seuil des pointeurs, boxé
/// au-delà (voir `box_int_if_needed`).
fn is_int(val: i64) -> bool {
    crate::is_int_box(val) || val < 0x10000
}

fn matches(val: i64, expected: &str) -> bool {
    match expected {
        "int" => is_int(val),
        "float" => crate::is_float_box(val) || is_int(val),
        "bool" => crate::is_bool_box(val) || val == 0 || val == 1,
        "array" => __is_array(val) != 0,
        "map" => __is_map(val) != 0,
        "function" => __is_function(val) != 0,
        _ => match expected.strip_prefix("class:") {
            Some(class) => __is_object(val) != 0 && class_chain(val).is_some_and(|chain| chain.iter().any(|c| c == class)),
            None => true,
        },
    }
}

/// `v` s'il est du type `expected`, sinon `ConvertException` (103).
#[unsafe(no_mangle)]
pub extern "C" fn Convert_mixedAs(val: i64, expected: i64) -> i64 {
    let expected = unsafe { crate::ptr_to_str(expected) };
    if matches(val, expected) {
        unsafe { crate::rc::retain(val) }
        return val;
    }
    let shown = expected.strip_prefix("class:").unwrap_or(expected);
    unsafe {
        crate::exception::throw_convert_exception(
            &format!("expected a value of type '{}', found '{}'", shown, found_name(val)),
            crate::ERR_CONVERT_TYPE,
            "Convert",
        )
    }
}
