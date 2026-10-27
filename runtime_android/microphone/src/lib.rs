//! `ocara.AndroidMicrophone` — enregistrement AAC par `MediaRecorder`
//! (`OcaraMicrophone.kt`).
#![allow(non_snake_case)]

use ocara_runtime_android_common::ffi::str_arg;
use ocara_runtime_android_common::java::{invoke, Arg, Ret};

const SOURCE: &str = "AndroidMicrophone";
const CLASS: &str = "com.ocara.bridge.OcaraMicrophone";

/// AndroidMicrophone::start(path)
#[unsafe(no_mangle)]
pub extern "C" fn AndroidMicrophone_start(path: i64) {
    invoke(SOURCE, CLASS, "start", &[Arg::Str(str_arg(path))], Ret::Void);
}

/// AndroidMicrophone::stop()
#[unsafe(no_mangle)]
pub extern "C" fn AndroidMicrophone_stop() {
    invoke(SOURCE, CLASS, "stop", &[], Ret::Void);
}

/// AndroidMicrophone::isRecording() → bool
#[unsafe(no_mangle)]
pub extern "C" fn AndroidMicrophone_isRecording() -> i64 {
    invoke(SOURCE, CLASS, "isRecording", &[], Ret::Bool).bool()
}
