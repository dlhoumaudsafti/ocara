//! `ocara.AndroidUI` — spécificités de l'interface Android (`OcaraUI.kt`) :
//! événements Kotlin → Ocara (bouton retour), barres système, orientation,
//! plein écran, écran maintenu allumé, vibration, partage, boîtes de
//! dialogue et messages brefs (noms standard d'`ocara.UI` : alert, confirm,
//! toast).
#![allow(non_snake_case)]

mod events;

use ocara_runtime_android_common::ffi::str_arg;
use ocara_runtime_android_common::java::{invoke, Arg, Ret};

const SOURCE: &str = "AndroidUI";
const CLASS: &str = "com.ocara.bridge.OcaraUI";

fn call(method: &str, args: &[Arg], ret: Ret) -> ocara_runtime_android_common::java::Value {
    invoke(SOURCE, CLASS, method, args, ret)
}

/// AndroidUI::listen(event, callback) — `callback` reçoit les données (JSON)
/// de l'événement. `"back"` : bouton retour, alors intercepté.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_listen(event: i64, callback: i64) {
    let event = str_arg(event);
    events::listen(event, callback);
    if event == "back" {
        call("setInterceptBack", &[Arg::Int(1)], Ret::Void);
    }
}

/// AndroidUI::setSystemBarsStyle(style) — `"light"` (icônes sombres) ou `"dark"`.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_setSystemBarsStyle(style: i64) {
    call("setSystemBarsStyle", &[Arg::Str(str_arg(style))], Ret::Void);
}

/// AndroidUI::setFullscreen(enabled) — masque les barres système (immersif).
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_setFullscreen(enabled: i64) {
    call("setFullscreen", &[Arg::Int(enabled)], Ret::Void);
}

/// AndroidUI::setOrientation(orientation) — `"portrait"`, `"landscape"` ou `"auto"`.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_setOrientation(orientation: i64) {
    call("setOrientation", &[Arg::Str(str_arg(orientation))], Ret::Void);
}

/// AndroidUI::setKeepScreenOn(enabled)
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_setKeepScreenOn(enabled: i64) {
    call("setKeepScreenOn", &[Arg::Int(enabled)], Ret::Void);
}

/// AndroidUI::vibrate(ms)
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_vibrate(ms: i64) {
    call("vibrate", &[Arg::Int(ms)], Ret::Void);
}

/// AndroidUI::share(text, title) — feuille de partage du système.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_share(text: i64, title: i64) {
    call("share", &[Arg::Str(str_arg(text)), Arg::Str(str_arg(title))], Ret::Void);
}

/// AndroidUI::alert(title, text) — bloque jusqu'à la fermeture.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_alert(title: i64, text: i64) {
    call("alert", &[Arg::Str(str_arg(title)), Arg::Str(str_arg(text))], Ret::Void);
}

/// AndroidUI::confirm(title, text) → bool
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_confirm(title: i64, text: i64) -> i64 {
    call("confirm", &[Arg::Str(str_arg(title)), Arg::Str(str_arg(text))], Ret::Bool).bool()
}

/// AndroidUI::toast(text) — message bref.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_toast(text: i64) {
    call("toast", &[Arg::Str(str_arg(text))], Ret::Void);
}

/// AndroidUI::close() — termine l'Activity.
#[unsafe(no_mangle)]
pub extern "C" fn AndroidUI_close() {
    call("close", &[], Ret::Void);
}
