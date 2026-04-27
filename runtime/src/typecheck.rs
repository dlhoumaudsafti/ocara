// ─────────────────────────────────────────────────────────────────────────────
// Type checking runtime pour le narrowing 'is Type'
// ─────────────────────────────────────────────────────────────────────────────

// Threshold : les valeurs < PTR_THRESHOLD sont des entiers directs
// les valeurs >= PTR_THRESHOLD sont des pointeurs
const PTR_THRESHOLD: i64 = 65536;

/// Check si une valeur est null (0)
#[no_mangle]
pub extern "C" fn __is_null(val: i64) -> i64 {
    if val == 0 { 1 } else { 0 }
}

/// Check si une valeur est un int
/// Heuristique : val != 0 && val < PTR_THRESHOLD
#[no_mangle]
pub extern "C" fn __is_int(val: i64) -> i64 {
    if val != 0 && val.abs() < PTR_THRESHOLD {
        1
    } else {
        0
    }
}

/// Check si une valeur est un float
/// Limitation : on ne peut pas distinguer un float d'un int sans boxing
/// Pour l'instant, retourne 0 (faux négatif conservateur)
#[no_mangle]
pub extern "C" fn __is_float(_val: i64) -> i64 {
    // TODO: nécessite un système de boxing avec tags
    0
}

/// Check si une valeur est un bool
/// Heuristique : val == 0 || val == 1
#[no_mangle]
pub extern "C" fn __is_bool(val: i64) -> i64 {
    if val == 0 || val == 1 {
        1
    } else {
        0
    }
}

/// Check si une valeur est une string
/// Heuristique : val >= PTR_THRESHOLD (c'est un pointeur)
#[no_mangle]
pub extern "C" fn __is_string(val: i64) -> i64 {
    if val >= PTR_THRESHOLD {
        1  // Probablement un pointeur (string, array, map, object)
    } else {
        0
    }
}

/// Check si une valeur est un array
/// Pour l'instant, même heuristique que string (pointeur)
/// TODO: ajouter des tags pour distinguer string/array/map/object
#[no_mangle]
pub extern "C" fn __is_array(val: i64) -> i64 {
    if val >= PTR_THRESHOLD {
        1
    } else {
        0
    }
}

/// Check si une valeur est une map
/// Pour l'instant, même heuristique que string (pointeur)
#[no_mangle]
pub extern "C" fn __is_map(val: i64) -> i64 {
    if val >= PTR_THRESHOLD {
        1
    } else {
        0
    }
}

/// Check si une valeur est un object (classe)
/// Pour l'instant, même heuristique que string (pointeur)
#[no_mangle]
pub extern "C" fn __is_object(val: i64) -> i64 {
    if val >= PTR_THRESHOLD {
        1
    } else {
        0
    }
}

/// Check si une valeur est une Function
/// Pour l'instant, même heuristique que string (pointeur)
#[no_mangle]
pub extern "C" fn __is_function(val: i64) -> i64 {
    if val >= PTR_THRESHOLD {
        1
    } else {
        0
    }
}
