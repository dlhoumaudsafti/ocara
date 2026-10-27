//! `ocara.AndroidFiles` — répertoires de l'app, assets de l'APK, dossier
//! Téléchargements partagé (`OcaraFiles.kt`).
#![allow(non_snake_case)]

use ocara_runtime_android_common::ffi::str_arg;
use ocara_runtime_android_common::java::{invoke, Arg, Ret};

const SOURCE: &str = "AndroidFiles";
const CLASS: &str = "com.ocara.bridge.OcaraFiles";

/// AndroidFiles::filesDir() → string
#[unsafe(no_mangle)]
pub extern "C" fn AndroidFiles_filesDir() -> i64 {
    invoke(SOURCE, CLASS, "filesDir", &[], Ret::Str).string()
}

/// AndroidFiles::cacheDir() → string
#[unsafe(no_mangle)]
pub extern "C" fn AndroidFiles_cacheDir() -> i64 {
    invoke(SOURCE, CLASS, "cacheDir", &[], Ret::Str).string()
}

/// AndroidFiles::externalFilesDir() → string
#[unsafe(no_mangle)]
pub extern "C" fn AndroidFiles_externalFilesDir() -> i64 {
    invoke(SOURCE, CLASS, "externalFilesDir", &[], Ret::Str).string()
}

/// AndroidFiles::listAssets(dir) → array<string>
#[unsafe(no_mangle)]
pub extern "C" fn AndroidFiles_listAssets(dir: i64) -> i64 {
    invoke(SOURCE, CLASS, "listAssets", &[Arg::Str(str_arg(dir))], Ret::Strs).strings()
}

/// AndroidFiles::readAsset(path) → string
#[unsafe(no_mangle)]
pub extern "C" fn AndroidFiles_readAsset(path: i64) -> i64 {
    invoke(SOURCE, CLASS, "readAsset", &[Arg::Str(str_arg(path))], Ret::Str).string()
}

/// AndroidFiles::copyAsset(path, dest)
#[unsafe(no_mangle)]
pub extern "C" fn AndroidFiles_copyAsset(path: i64, dest: i64) {
    invoke(SOURCE, CLASS, "copyAsset", &[Arg::Str(str_arg(path)), Arg::Str(str_arg(dest))], Ret::Void);
}

/// AndroidFiles::saveToDownloads(source, name) → string (URI ou chemin)
#[unsafe(no_mangle)]
pub extern "C" fn AndroidFiles_saveToDownloads(source: i64, name: i64) -> i64 {
    invoke(SOURCE, CLASS, "saveToDownloads", &[Arg::Str(str_arg(source)), Arg::Str(str_arg(name))], Ret::Str).string()
}
