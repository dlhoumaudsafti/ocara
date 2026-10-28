// src/codegen/desc.d/android.rs — signatures Cranelift des builtins
// `ocara.Android*` (crates runtime_android/<domaine>). Hors Android, chaque
// fonction est un talon local qui lève AndroidException 100 (emitter.rs).

use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

const I: clt::Type = clt::I64;

const fn desc(module: &'static str, name: &'static str, params: &'static [clt::Type], returns: Option<clt::Type>) -> BuiltinDesc {
    BuiltinDesc { name, params, returns, module: Some(module) }
}

pub const ANDROID_BUILTINS: &[BuiltinDesc] = &[
    desc("AndroidPermissions", "AndroidPermissions_isGranted", &[I], Some(I)),
    desc("AndroidPermissions", "AndroidPermissions_request", &[I], Some(I)),
    desc("AndroidNotifications", "AndroidNotifications_show", &[I, I], Some(I)),
    desc("AndroidNotifications", "AndroidNotifications_cancel", &[I], None),
    desc("AndroidNotifications", "AndroidNotifications_areEnabled", &[], Some(I)),
    desc("AndroidSensors", "AndroidSensors_isAvailable", &[I], Some(I)),
    desc("AndroidSensors", "AndroidSensors_list", &[], Some(I)),
    desc("AndroidSensors", "AndroidSensors_read", &[I], Some(I)),
    desc("AndroidFiles", "AndroidFiles_filesDir", &[], Some(I)),
    desc("AndroidFiles", "AndroidFiles_cacheDir", &[], Some(I)),
    desc("AndroidFiles", "AndroidFiles_externalFilesDir", &[], Some(I)),
    desc("AndroidFiles", "AndroidFiles_listAssets", &[I], Some(I)),
    desc("AndroidFiles", "AndroidFiles_readAsset", &[I], Some(I)),
    desc("AndroidFiles", "AndroidFiles_copyAsset", &[I, I], None),
    desc("AndroidFiles", "AndroidFiles_saveToDownloads", &[I, I], Some(I)),
    desc("AndroidCamera", "AndroidCamera_count", &[], Some(I)),
    desc("AndroidCamera", "AndroidCamera_takePhoto", &[I, I], None),
    desc("AndroidMicrophone", "AndroidMicrophone_start", &[I], None),
    desc("AndroidMicrophone", "AndroidMicrophone_stop", &[], None),
    desc("AndroidMicrophone", "AndroidMicrophone_isRecording", &[], Some(I)),
    desc("AndroidAudio", "AndroidAudio_play", &[I], None),
    desc("AndroidAudio", "AndroidAudio_pause", &[], None),
    desc("AndroidAudio", "AndroidAudio_resume", &[], None),
    desc("AndroidAudio", "AndroidAudio_stop", &[], None),
    desc("AndroidAudio", "AndroidAudio_isPlaying", &[], Some(I)),
    desc("AndroidAudio", "AndroidAudio_setVolume", &[clt::F64], None),
    desc("AndroidUI", "AndroidUI_listen", &[I, I], None),
    desc("AndroidUI", "AndroidUI_setSystemBarsStyle", &[I], None),
    desc("AndroidUI", "AndroidUI_setFullscreen", &[I], None),
    desc("AndroidUI", "AndroidUI_setOrientation", &[I], None),
    desc("AndroidUI", "AndroidUI_setKeepScreenOn", &[I], None),
    desc("AndroidUI", "AndroidUI_vibrate", &[I], None),
    desc("AndroidUI", "AndroidUI_share", &[I, I], None),
    desc("AndroidUI", "AndroidUI_alert", &[I, I], None),
    desc("AndroidUI", "AndroidUI_confirm", &[I, I], Some(I)),
    desc("AndroidUI", "AndroidUI_toast", &[I], None),
    desc("AndroidUI", "AndroidUI_close", &[], None),
];
