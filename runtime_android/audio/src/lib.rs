//! `ocara.AndroidAudio` — lecture d'un fichier audio par `MediaPlayer`
//! (`OcaraAudio.kt`).
#![allow(non_snake_case)]

use ocara_runtime_android_common::ffi::str_arg;
use ocara_runtime_android_common::java::{invoke, Arg, Ret};

const SOURCE: &str = "AndroidAudio";
const CLASS: &str = "com.ocara.bridge.OcaraAudio";

/// AndroidAudio::play(path)
#[unsafe(no_mangle)]
pub extern "C" fn AndroidAudio_play(path: i64) {
    invoke(SOURCE, CLASS, "play", &[Arg::Str(str_arg(path))], Ret::Void);
}

/// AndroidAudio::pause()
#[unsafe(no_mangle)]
pub extern "C" fn AndroidAudio_pause() {
    invoke(SOURCE, CLASS, "pause", &[], Ret::Void);
}

/// AndroidAudio::resume()
#[unsafe(no_mangle)]
pub extern "C" fn AndroidAudio_resume() {
    invoke(SOURCE, CLASS, "resume", &[], Ret::Void);
}

/// AndroidAudio::stop()
#[unsafe(no_mangle)]
pub extern "C" fn AndroidAudio_stop() {
    invoke(SOURCE, CLASS, "stop", &[], Ret::Void);
}

/// AndroidAudio::isPlaying() → bool
#[unsafe(no_mangle)]
pub extern "C" fn AndroidAudio_isPlaying() -> i64 {
    invoke(SOURCE, CLASS, "isPlaying", &[], Ret::Bool).bool()
}

/// AndroidAudio::setVolume(volume) — de 0.0 à 1.0.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidAudio_setVolume(volume: f64) {
    invoke(SOURCE, CLASS, "setVolume", &[Arg::Float(volume)], Ret::Void);
}
