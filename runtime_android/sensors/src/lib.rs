//! `ocara.AndroidSensors` — lecture des capteurs par l'API NDK
//! `ASensorManager` (sans Java, `-landroid`).
#![allow(non_snake_case)]

mod ndk;

use ocara_runtime_android_common::ffi::{float_array, str_arg, string_array};
use ocara_runtime_android_common::{Failure, ERR_ARGUMENT};

const SOURCE: &str = "AndroidSensors";

/// Nom Ocara, type NDK (`ASENSOR_TYPE_*`), nombre de valeurs d'un relevé.
const SENSORS: &[(&str, i32, usize)] = &[
    ("accelerometer", 1, 3),
    ("magnetic_field", 2, 3),
    ("gyroscope", 4, 3),
    ("light", 5, 1),
    ("pressure", 6, 1),
    ("proximity", 8, 1),
    ("gravity", 9, 3),
    ("linear_acceleration", 10, 3),
    ("rotation_vector", 11, 4),
    ("relative_humidity", 12, 1),
    ("ambient_temperature", 13, 1),
    ("step_counter", 19, 1),
];

fn sensor(name: &str) -> (i32, usize) {
    SENSORS.iter().find(|(n, _, _)| *n == name).map(|&(_, kind, count)| (kind, count)).unwrap_or_else(|| {
        let known: Vec<&str> = SENSORS.iter().map(|(n, _, _)| *n).collect();
        Failure::new(ERR_ARGUMENT, format!("capteur inconnu : '{name}' (connus : {})", known.join(", "))).raise(SOURCE)
    })
}

/// AndroidSensors::isAvailable(sensor) → bool
#[unsafe(no_mangle)]
pub extern "C" fn AndroidSensors_isAvailable(name: i64) -> i64 {
    ndk::available(sensor(str_arg(name)).0) as i64
}

/// AndroidSensors::list() → array<string>, capteurs présents sur l'appareil.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidSensors_list() -> i64 {
    let present: Vec<String> = SENSORS.iter()
        .filter(|(_, kind, _)| ndk::available(*kind))
        .map(|(name, _, _)| name.to_string())
        .collect();
    string_array(&present)
}

/// AndroidSensors::read(sensor) → array<float>, un relevé.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidSensors_read(name: i64) -> i64 {
    let name = str_arg(name);
    let (kind, count) = sensor(name);
    match ndk::read(kind, count) {
        Ok(values) => float_array(&values),
        Err(failure) => Failure::new(failure.code, format!("{name} : {}", failure.message)).raise(SOURCE),
    }
}
