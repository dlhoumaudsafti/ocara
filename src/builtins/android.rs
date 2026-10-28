// ─────────────────────────────────────────────────────────────────────────────
// ocara.Android* — fonctionnalités natives Android hors GUI, un builtin par
// domaine. Chacun a son crate (runtime_android/<domaine>), lié dans le .so
// seulement si le builtin est importé ; hors Android, chaque appel lève
// AndroidException 100 (talon, src/codegen/emit.d/emitter.rs).
//
//   AndroidPermissions::isGranted(permission) / request(permission) → bool
//   AndroidNotifications::show(title, text) → int / cancel(id) / areEnabled() → bool
//   AndroidSensors::isAvailable(sensor) → bool / list() → array<string> / read(sensor) → array<float>
//   AndroidFiles::filesDir() / cacheDir() / externalFilesDir() → string
//   AndroidFiles::listAssets(dir) / readAsset(path) / copyAsset(path, dest) / saveToDownloads(source, name)
//   AndroidCamera::count() → int / takePhoto(path, facing)
//   AndroidMicrophone::start(path) / stop() / isRecording() → bool
//   AndroidAudio::play(path) / pause() / resume() / stop() / isPlaying() → bool / setVolume(volume)
//   AndroidUI::listen(event, callback) / setSystemBarsStyle(style) / setFullscreen(enabled)
//   AndroidUI::setOrientation(o) / setKeepScreenOn(enabled) / vibrate(ms) / share(text, title)
//   AndroidUI::alert(title, text) / confirm(title, text) → bool / toast(text) / close()
//
// Convention runtime : <Classe>_<méthode>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

/// Builtins `ocara.Android*` et le domaine de chacun (crate
/// `runtime_android/<domaine>`, archive `libocara_runtime_android_<domaine>.a`).
pub const ANDROID_BUILTINS: &[(&str, &str)] = &[
    ("AndroidPermissions", "permissions"),
    ("AndroidNotifications", "notifications"),
    ("AndroidSensors", "sensors"),
    ("AndroidFiles", "files"),
    ("AndroidCamera", "camera"),
    ("AndroidMicrophone", "microphone"),
    ("AndroidAudio", "audio"),
    ("AndroidUI", "ui"),
];

fn static_m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
        message_emit_in_loop: false,
    }
}

fn class_of(methods: HashMap<String, FuncSig>) -> ClassInfo {
    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}

pub fn permissions_class() -> ClassInfo {
    let mut methods = HashMap::new();
    methods.insert("isGranted".to_string(), static_m(vec![("permission", Type::String)], Type::Bool));
    methods.insert("request".to_string(), static_m(vec![("permission", Type::String)], Type::Bool));
    class_of(methods)
}

pub fn notifications_class() -> ClassInfo {
    let mut methods = HashMap::new();
    methods.insert("show".to_string(), static_m(vec![("title", Type::String), ("text", Type::String)], Type::Int));
    methods.insert("cancel".to_string(), static_m(vec![("id", Type::Int)], Type::Void));
    methods.insert("areEnabled".to_string(), static_m(vec![], Type::Bool));
    class_of(methods)
}

pub fn sensors_class() -> ClassInfo {
    let mut methods = HashMap::new();
    methods.insert("isAvailable".to_string(), static_m(vec![("sensor", Type::String)], Type::Bool));
    methods.insert("list".to_string(), static_m(vec![], Type::Array(Box::new(Type::String))));
    methods.insert("read".to_string(), static_m(vec![("sensor", Type::String)], Type::Array(Box::new(Type::Float))));
    class_of(methods)
}

pub fn files_class() -> ClassInfo {
    let mut methods = HashMap::new();
    methods.insert("filesDir".to_string(), static_m(vec![], Type::String));
    methods.insert("cacheDir".to_string(), static_m(vec![], Type::String));
    methods.insert("externalFilesDir".to_string(), static_m(vec![], Type::String));
    methods.insert("listAssets".to_string(), static_m(vec![("dir", Type::String)], Type::Array(Box::new(Type::String))));
    methods.insert("readAsset".to_string(), static_m(vec![("path", Type::String)], Type::String));
    methods.insert("copyAsset".to_string(), static_m(vec![("path", Type::String), ("dest", Type::String)], Type::Void));
    methods.insert("saveToDownloads".to_string(), static_m(vec![("source", Type::String), ("name", Type::String)], Type::String));
    class_of(methods)
}

pub fn camera_class() -> ClassInfo {
    let mut methods = HashMap::new();
    methods.insert("count".to_string(), static_m(vec![], Type::Int));
    methods.insert("takePhoto".to_string(), static_m(vec![("path", Type::String), ("facing", Type::String)], Type::Void));
    class_of(methods)
}

pub fn microphone_class() -> ClassInfo {
    let mut methods = HashMap::new();
    methods.insert("start".to_string(), static_m(vec![("path", Type::String)], Type::Void));
    methods.insert("stop".to_string(), static_m(vec![], Type::Void));
    methods.insert("isRecording".to_string(), static_m(vec![], Type::Bool));
    class_of(methods)
}

pub fn audio_class() -> ClassInfo {
    let mut methods = HashMap::new();
    methods.insert("play".to_string(), static_m(vec![("path", Type::String)], Type::Void));
    methods.insert("pause".to_string(), static_m(vec![], Type::Void));
    methods.insert("resume".to_string(), static_m(vec![], Type::Void));
    methods.insert("stop".to_string(), static_m(vec![], Type::Void));
    methods.insert("isPlaying".to_string(), static_m(vec![], Type::Bool));
    methods.insert("setVolume".to_string(), static_m(vec![("volume", Type::Float)], Type::Void));
    class_of(methods)
}

pub fn ui_class() -> ClassInfo {
    let mut methods = HashMap::new();
    let callback = Type::Function { ret_ty: Box::new(Type::Void), param_tys: vec![Type::String] };
    methods.insert("listen".to_string(), static_m(vec![("event", Type::String), ("callback", callback)], Type::Void));
    methods.insert("setSystemBarsStyle".to_string(), static_m(vec![("style", Type::String)], Type::Void));
    methods.insert("setFullscreen".to_string(), static_m(vec![("enabled", Type::Bool)], Type::Void));
    methods.insert("setOrientation".to_string(), static_m(vec![("orientation", Type::String)], Type::Void));
    methods.insert("setKeepScreenOn".to_string(), static_m(vec![("enabled", Type::Bool)], Type::Void));
    methods.insert("vibrate".to_string(), static_m(vec![("ms", Type::Int)], Type::Void));
    methods.insert("share".to_string(), static_m(vec![("text", Type::String), ("title", Type::String)], Type::Void));
    methods.insert("alert".to_string(), static_m(vec![("title", Type::String), ("text", Type::String)], Type::Void));
    methods.insert("confirm".to_string(), static_m(vec![("title", Type::String), ("text", Type::String)], Type::Bool));
    methods.insert("toast".to_string(), static_m(vec![("text", Type::String)], Type::Void));
    methods.insert("close".to_string(), static_m(vec![], Type::Void));
    class_of(methods)
}

/// Classe de l'un des builtins `ANDROID_BUILTINS`.
pub fn class(name: &str) -> Option<ClassInfo> {
    match name {
        "AndroidPermissions" => Some(permissions_class()),
        "AndroidNotifications" => Some(notifications_class()),
        "AndroidSensors" => Some(sensors_class()),
        "AndroidFiles" => Some(files_class()),
        "AndroidCamera" => Some(camera_class()),
        "AndroidMicrophone" => Some(microphone_class()),
        "AndroidAudio" => Some(audio_class()),
        "AndroidUI" => Some(ui_class()),
        _ => None,
    }
}
