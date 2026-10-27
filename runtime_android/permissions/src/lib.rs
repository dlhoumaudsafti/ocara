//! `ocara.AndroidPermissions` — permissions d'exécution (`OcaraPermissions.kt`).
#![allow(non_snake_case)]

use ocara_runtime_android_common::ffi::str_arg;
use ocara_runtime_android_common::java::{invoke, Arg, Ret};

const SOURCE: &str = "AndroidPermissions";
const CLASS: &str = "com.ocara.bridge.OcaraPermissions";

/// AndroidPermissions::isGranted(permission) → bool
#[unsafe(no_mangle)]
pub extern "C" fn AndroidPermissions_isGranted(permission: i64) -> i64 {
    invoke(SOURCE, CLASS, "isGranted", &[Arg::Str(str_arg(permission))], Ret::Bool).bool()
}

/// AndroidPermissions::request(permission) → bool, bloquant jusqu'à la réponse.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidPermissions_request(permission: i64) -> i64 {
    invoke(SOURCE, CLASS, "request", &[Arg::Str(str_arg(permission))], Ret::Bool).bool()
}
