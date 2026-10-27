//! Accès par clé à une valeur `mixed` (`x["clé"]`, `x[0]`, `x.clé`) :
//! `Convert::mixedGet`/`Convert::mixedSet`, insérés par le compilateur quand
//! le type statique du receveur est `mixed`. Map ou tableau selon l'étiquette
//! du bloc ; une valeur nulle se lit nulle et ne s'écrit pas (comme
//! `m["k"]` sur une map nulle) ; toute autre valeur lève une
//! `ConvertException` (103). Le lowering choisissait jusqu'ici `__array_get`
//! faute de type statique : une map lue ainsi rendait `null`.

use crate::mixed_check::{found_name, matches};
use crate::typecheck::{__is_array, __is_map};

/// Indice d'un tableau porté par un `mixed` : entier brut ou boxé.
fn array_index(container: i64, key: i64) -> i64 {
    if crate::is_int_box(key) {
        return unsafe { crate::unbox_int(key) };
    }
    if key < 0x10000 {
        return key;
    }
    type_error(&format!("an array index must be an int, found '{}' (container: {})", found_name(key), found_name(container)))
}

/// Élément lu, rendu comme `mixed` possédé : retenu, ou boxé selon le genre
/// des éléments bruts (`map<string, int>`…).
fn elem_to_mixed(container: i64, elem: i64) -> i64 {
    match crate::rc::raw_kind(container) {
        1 => crate::box_int_if_needed(elem),
        2 => crate::__box_float(elem),
        3 => crate::__box_bool(elem),
        _ => {
            unsafe { crate::rc::retain(elem) };
            elem
        }
    }
}

/// Valeur `mixed` à ranger dans `container`, au format de ses éléments.
fn mixed_to_elem(container: i64, value: i64) -> i64 {
    let (kind, name) = match crate::rc::raw_kind(container) {
        0 => return value,
        1 => (1, "int"),
        2 => (2, "float"),
        _ => (3, "bool"),
    };
    if !matches(value, name) {
        type_error(&format!("expected a value of type '{}', found '{}'", name, found_name(value)));
    }
    match kind {
        1 => crate::__mixed_to_int(value),
        2 => crate::__mixed_to_float(value).to_bits() as i64,
        _ => i64::from(if crate::is_bool_box(value) { unsafe { crate::unbox_bool(value) } } else { value != 0 }),
    }
}

fn type_error(message: &str) -> ! {
    unsafe { crate::exception::throw_convert_exception(message, crate::ERR_CONVERT_TYPE, "Convert") }
}

fn not_indexable(val: i64) -> ! {
    type_error(&format!("cannot index a value of type '{}' — only arrays and maps can be indexed", found_name(val)))
}

/// `x[key]` / `x.key` sur un `mixed` : valeur possédée par l'appelant.
#[unsafe(no_mangle)]
pub extern "C" fn Convert_mixedGet(val: i64, key: i64) -> i64 {
    if val == 0 {
        return 0;
    }
    if __is_map(val) != 0 {
        return elem_to_mixed(val, crate::__map_get(val, key));
    }
    if __is_array(val) != 0 {
        return elem_to_mixed(val, crate::__array_get(val, array_index(val, key)));
    }
    not_indexable(val)
}

/// `x[key] = value` / `x.key = value` sur un `mixed`.
#[unsafe(no_mangle)]
pub extern "C" fn Convert_mixedSet(val: i64, key: i64, value: i64) {
    if val == 0 {
        return;
    }
    if __is_map(val) != 0 {
        return crate::__map_set(val, key, mixed_to_elem(val, value));
    }
    if __is_array(val) != 0 {
        return crate::__array_set(val, array_index(val, key), mixed_to_elem(val, value));
    }
    not_indexable(val)
}
