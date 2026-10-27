//! `ocara.AndroidCamera` — photo sans aperçu par Camera2 (`OcaraCamera.kt`).
#![allow(non_snake_case)]

use ocara_runtime_android_common::ffi::str_arg;
use ocara_runtime_android_common::java::{invoke, Arg, Ret};

const SOURCE: &str = "AndroidCamera";
const CLASS: &str = "com.ocara.bridge.OcaraCamera";

/// AndroidCamera::count() → int
#[unsafe(no_mangle)]
pub extern "C" fn AndroidCamera_count() -> i64 {
    invoke(SOURCE, CLASS, "count", &[], Ret::Int).int()
}

/// AndroidCamera::takePhoto(path, facing) — JPEG, `facing` "back" ou "front".
#[unsafe(no_mangle)]
pub extern "C" fn AndroidCamera_takePhoto(path: i64, facing: i64) {
    invoke(SOURCE, CLASS, "takePhoto", &[Arg::Str(str_arg(path)), Arg::Str(str_arg(facing))], Ret::Void);
}
