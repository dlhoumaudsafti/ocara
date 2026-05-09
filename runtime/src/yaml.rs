// ─────────────────────────────────────────────────────────────────────────────
// runtime/src/yaml.rs — Implémentation YAML
// ─────────────────────────────────────────────────────────────────────────────

use serde_yaml::{Value as YamlValue, Mapping as YamlMap};
use crate::{alloc_str, ptr_to_str, get_value_type, OcaraMap};
use crate::{__array_new, __array_len, __array_get, __array_push};
use crate::{__map_new, __map_set};

/// YAML::encode(data) → string
/// Encode un array ou map en YAML
#[unsafe(no_mangle)]
pub unsafe extern "C" fn YAML_encode(data: i64) -> i64 {
    unsafe {
        if data == 0 {
            return alloc_str("null\n");
        }
        
        let typ = get_value_type(data);
        
        match typ {
            5 => {  // TAG_ARRAY
                encode_array_to_yaml(data)
            }
            6 => {  // TAG_MAP
                encode_map_to_yaml(data)
            }
            _ => {
                // Type non supporté, retourner chaîne vide
                alloc_str("")
            }
        }
    }
}

/// Encode récursivement un array Ocara en YAML
fn encode_array_to_yaml(arr: i64) -> i64 {
    let mut yaml_arr = Vec::new();
    let len = __array_len(arr);
    
    for i in 0..len {
        let elem = __array_get(arr, i);
        let yaml_val = value_to_yaml(elem);
        yaml_arr.push(yaml_val);
    }
    
    let yaml_str = serde_yaml::to_string(&yaml_arr).unwrap_or_else(|_| "[]\n".to_string());
    unsafe { alloc_str(&yaml_str) }
}

/// Encode récursivement une map Ocara en YAML
fn encode_map_to_yaml(map: i64) -> i64 {
    let mut yaml_obj = YamlMap::new();
    
    unsafe {
        let map_ptr = map as *mut OcaraMap;
        for (key_str, value) in (*map_ptr).data.iter() {
            let yaml_val = value_to_yaml(*value);
            yaml_obj.insert(YamlValue::String(key_str.clone()), yaml_val);
        }
    }
    
    let yaml_str = serde_yaml::to_string(&yaml_obj).unwrap_or_else(|_| "{}\n".to_string());
    unsafe { alloc_str(&yaml_str) }
}

/// Convertit une valeur Ocara en YamlValue
fn value_to_yaml(val: i64) -> YamlValue {
    if val == 0 {
        return YamlValue::Null;
    }
    
    let typ = get_value_type(val);
    
    match typ {
        1 => {  // Primitif (int ou bool)
            if val == 1 {
                YamlValue::Bool(true)
            } else if val == 0 {
                YamlValue::Bool(false)
            } else {
                YamlValue::Number(serde_yaml::Number::from(val))
            }
        }
        4 => {  // String
            YamlValue::String(unsafe { ptr_to_str(val) }.to_string())
        }
        5 => {  // Array
            let mut yaml_arr = Vec::new();
            let len = __array_len(val);
            for i in 0..len {
                let elem = __array_get(val, i);
                yaml_arr.push(value_to_yaml(elem));
            }
            YamlValue::Sequence(yaml_arr)
        }
        6 => {  // Map
            let mut yaml_obj = YamlMap::new();
            unsafe {
                let map_ptr = val as *mut OcaraMap;
                for (key_str, value) in (*map_ptr).data.iter() {
                    yaml_obj.insert(YamlValue::String(key_str.clone()), value_to_yaml(*value));
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
                i
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
