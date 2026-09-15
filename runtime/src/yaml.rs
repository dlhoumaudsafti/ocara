// ─────────────────────────────────────────────────────────────────────────────
// runtime/src/yaml.rs — Implémentation YAML
// ─────────────────────────────────────────────────────────────────────────────

use serde_yaml::{Value as YamlValue, Mapping as YamlMap};
use crate::{alloc_str, ptr_to_str, get_value_type, OcaraMap};
use crate::{__array_new, __array_len, __array_get, __array_push};
use crate::{__map_new, __map_set};

/// YAML::encode(data, leaf_kind) → string
/// Encode un array ou map en YAML. `leaf_kind` (2e paramètre, jamais visible
/// côté langage Ocara — voir `static_json_leaf_kind` côté lowering, réutilisé
/// tel quel pour YAML) : 0 = inconnu/`mixed` (comportement heuristique
/// historique), 1/2/3 = int/float/bool — le type de feuille concret d'un
/// conteneur qui ne boxe jamais ses éléments (`array<int>`...), sans quoi un
/// entier brut `0`/`1` est indiscernable de `null`/`false` (voir
/// `value_to_yaml`, même angle mort que pour JSON — voir
/// docs/roadmap.d/langage-mixed-literal-stringification.md).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn YAML_encode(data: i64, leaf_kind: i64) -> i64 {
    let yaml_val = value_to_yaml(data, leaf_kind);
    let yaml_str = serde_yaml::to_string(&yaml_val).unwrap_or_else(|_| "null\n".to_string());
    unsafe { alloc_str(&yaml_str) }
}

/// Convertit une valeur Ocara en YamlValue. `leaf_kind` : voir `YAML_encode`
/// — reste le MÊME à travers toute la récursion (même raisonnement que
/// `value_to_json` dans lib.rs) et ne s'applique QUE lorsque `val` n'est pas
/// lui-même un pointeur tas réel (seul le panier "primitif", jamais un
/// pointeur valide, est concerné par l'ambiguïté que `leaf_kind` résout).
fn value_to_yaml(val: i64, leaf_kind: i64) -> YamlValue {
    if val == 0 {
        // `null` uniquement pour `mixed` (leaf_kind == 0) — pour un type de
        // feuille concret connu, 0 est une valeur int/float/bool normale.
        return match leaf_kind {
            1 => YamlValue::Number(serde_yaml::Number::from(0i64)),
            2 => YamlValue::Number(serde_yaml::Number::from(0.0)),
            3 => YamlValue::Bool(false),
            _ => YamlValue::Null,
        };
    }

    let typ = get_value_type(val);

    match typ {
        1 => {  // Primitif : jamais un pointeur valide — résolu directement
                // par `leaf_kind` quand il est connu (aucune approximation
                // possible), sinon heuristique `mixed` historique.
            match leaf_kind {
                1 => YamlValue::Number(serde_yaml::Number::from(val)),
                2 => YamlValue::Number(serde_yaml::Number::from(f64::from_bits(val as u64))),
                3 => YamlValue::Bool(val != 0),
                _ => {
                    // Bool BOXÉ (tag bits 1:0 = 10) : à vérifier AVANT le bool
                    // brut ci-dessous — `__unbox_bool` déréférence un pointeur,
                    // jamais sûr à appeler sur un 0/1 brut qui n'est PAS un
                    // pointeur boxé.
                    const PTR_THRESHOLD: i64 = 65536;
                    let is_boxed_bool = val >= PTR_THRESHOLD && (val & 3) == 2;
                    // Int BOXÉ (tag bits 1:0 = 11, voir `box_int_if_needed`
                    // dans lib.rs) : un entier assez grand pour être ambigu
                    // avec un pointeur heap une fois logé dans un `mixed` — à
                    // vérifier AVANT le fallback `YamlValue::Number(val)`
                    // ci-dessous, qui prendrait sinon l'adresse boxée
                    // elle-même pour la valeur.
                    let is_boxed_int = val >= PTR_THRESHOLD && (val & 3) == 3;
                    if crate::typecheck::__is_float(val) != 0 {
                        YamlValue::Number(serde_yaml::Number::from(crate::__unbox_float(val)))
                    } else if is_boxed_bool {
                        YamlValue::Bool(crate::__unbox_bool(val) != 0)
                    } else if is_boxed_int {
                        YamlValue::Number(serde_yaml::Number::from(crate::__unbox_int(val)))
                    } else if val == 1 {
                        YamlValue::Bool(true)
                    } else {
                        YamlValue::Number(serde_yaml::Number::from(val))
                    }
                }
            }
        }
        4 => {  // String
            YamlValue::String(unsafe { ptr_to_str(val) }.to_string())
        }
        5 => {  // Array — même leaf_kind à travers toute la récursion.
            let mut yaml_arr = Vec::new();
            let len = __array_len(val);
            for i in 0..len {
                let elem = __array_get(val, i);
                yaml_arr.push(value_to_yaml(elem, leaf_kind));
            }
            YamlValue::Sequence(yaml_arr)
        }
        6 => {  // Map — même leaf_kind à travers toute la récursion.
            let mut yaml_obj = YamlMap::new();
            unsafe {
                let map_ptr = val as *mut OcaraMap;
                for (key_str, value) in (*map_ptr).data.iter() {
                    yaml_obj.insert(YamlValue::String(key_str.clone()), value_to_yaml(*value, leaf_kind));
                }
            }
            YamlValue::Mapping(yaml_obj)
        }
        _ => YamlValue::Null
    }
}

/// YAML::decode(yaml) → mixed
/// Décode une string YAML en structure Ocara
#[unsafe(no_mangle)]
pub unsafe extern "C" fn YAML_decode(yaml: i64) -> i64 {
    unsafe {
        if yaml == 0 {
            return 0;
        }
        
        let yaml_str = ptr_to_str(yaml);
        
        match serde_yaml::from_str::<YamlValue>(yaml_str) {
            Ok(value) => yaml_to_value(&value),
            Err(_) => 0  // Retourner null en cas d'erreur
        }
    }
}

/// YAML::parse(yaml) → mixed (alias de decode)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn YAML_parse(yaml: i64) -> i64 {
    unsafe {
        YAML_decode(yaml)
    }
}

/// Convertit un YamlValue en valeur Ocara
fn yaml_to_value(yaml: &YamlValue) -> i64 {
    match yaml {
        YamlValue::Null => 0,
        YamlValue::Bool(b) => if *b { 1 } else { 0 },
        YamlValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                // Toujours logé dans un `mixed` (élément d'array/map) : boxer
                // si besoin, comme n'importe quel autre `int` en transit vers
                // un `mixed` (voir `box_int_if_needed` dans lib.rs).
                crate::__box_int_for_mixed(i)
            } else if let Some(f) = n.as_f64() {
                // Boxé comme un float `mixed` (voir __box_float) : un entier
                // brut n'aurait pas atteint cette branche (as_i64() aurait
                // réussi), donc n a réellement une partie décimale.
                crate::__box_float(f.to_bits() as i64)
            } else {
                0
            }
        }
        YamlValue::String(s) => unsafe { alloc_str(s) },
        YamlValue::Sequence(arr) => {
            let ocara_arr = __array_new();
            for elem in arr {
                let ocara_val = yaml_to_value(elem);
                __array_push(ocara_arr, ocara_val);
            }
            ocara_arr
        }
        YamlValue::Mapping(obj) => {
            let ocara_map = __map_new();
            for (key, value) in obj {
                // Convertir la clé en string
                let key_str = match key {
                    YamlValue::String(s) => unsafe { alloc_str(s) },
                    _ => unsafe { alloc_str("") },
                };
                let ocara_val = yaml_to_value(value);
                __map_set(ocara_map, key_str, ocara_val);
            }
            ocara_map
        }
        _ => 0  // Types YAML non supportés (tagged, etc.)
    }
}
