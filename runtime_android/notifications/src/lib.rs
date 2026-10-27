//! `ocara.AndroidNotifications` — notifications système (`OcaraNotifications.kt`).
#![allow(non_snake_case)]

use ocara_runtime_android_common::ffi::str_arg;
use ocara_runtime_android_common::java::{invoke, Arg, Ret};

const SOURCE: &str = "AndroidNotifications";
const CLASS: &str = "com.ocara.bridge.OcaraNotifications";

/// AndroidNotifications::show(title, text) → int (identifiant)
#[unsafe(no_mangle)]
pub extern "C" fn AndroidNotifications_show(title: i64, text: i64) -> i64 {
    invoke(SOURCE, CLASS, "show", &[Arg::Str(str_arg(title)), Arg::Str(str_arg(text))], Ret::Int).int()
}

/// AndroidNotifications::cancel(id)
#[unsafe(no_mangle)]
pub extern "C" fn AndroidNotifications_cancel(id: i64) {
    invoke(SOURCE, CLASS, "cancel", &[Arg::Int(id)], Ret::Void);
}

/// AndroidNotifications::areEnabled() → bool
#[unsafe(no_mangle)]
pub extern "C" fn AndroidNotifications_areEnabled() -> i64 {
    invoke(SOURCE, CLASS, "areEnabled", &[], Ret::Bool).bool()
}
