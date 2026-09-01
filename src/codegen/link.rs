use std::path::{Path, PathBuf};
use std::process::Command;

// ─────────────────────────────────────────────────────────────────────────────
// Liaison finale : fichier objet → exécutable natif
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct LinkerError(pub String);

impl std::fmt::Display for LinkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "linker error: {}", self.0)
    }
}
impl std::error::Error for LinkerError {}

/// Runtime principal, embarqué dans le binaire du compilateur (build.rs + include_bytes!).
static RUNTIME_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libocara_runtime.a"));

/// Runtime Tauri, dans un crate SÉPARÉ du runtime principal (voir runtime_tauri/
/// Cargo.toml). Raison : rustc partitionne son code en unités de compilation sans
/// respecter les frontières de module — même avec `-Wl,--gc-sections` au lien, du
/// code toujours nécessaire (ex: thread.rs) peut finir dans la même unité qu'une
/// fonction Tauri, ce qui la garde vivante et exige GTK/WebKit installés sur la
/// machine cible pour un programme qui n'utilise même pas Tauri (mesuré : ~100
/// bibliothèques dynamiques / 42 Mo pour un simple "Hello World"). Un crate séparé
/// donne une frontière de lien dure : ce .a n'est extrait et lié que pour les
/// programmes qui importent réellement ocara.Tauri (voir `needs_tauri` ci-dessous).
/// Résultat mesuré sans Tauri : 7 bibliothèques dynamiques / 6 Mo.
static RUNTIME_TAURI_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libocara_runtime_tauri.a"));

/// Runtime SDL, même principe et même raison que RUNTIME_TAURI_BYTES ci-dessus
/// (voir runtime_sdl/Cargo.toml) : crate séparé pour que SDL3 (et sa chaîne de
/// build vendored — cmake, X11/Wayland dev) ne soit lié que pour les programmes
/// qui importent réellement ocara.SDL. Contrairement à Tauri, SDL3 est compilé
/// et lié STATIQUEMENT (feature `build-from-source-static`) — aucune bibliothèque
/// dynamique supplémentaire à ajouter au lien final (pas de pkg-config ici).
static RUNTIME_SDL_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libocara_runtime_sdl.a"));

/// Extrait des bytes embarqués vers un fichier temporaire et retourne son chemin.
fn extract_to_tmp(bytes: &[u8], label: &str) -> Result<PathBuf, LinkerError> {
    let path = std::env::temp_dir()
        .join(format!("lib{}_{}.a", label, std::process::id()));
    std::fs::write(&path, bytes)
        .map_err(|e| LinkerError(format!("extraction de {}: {}", label, e)))?;
    Ok(path)
}

/// Résout les flags `--libs` de pkg-config pour un paquet, avec repli automatique
/// `<nom>-4.0` → `<nom>-4.1` (Ubuntu ≥ 24.04 n'a plus les `.pc` en 4.0 — voir le
/// même repli côté Makefile pour la compilation du runtime lui-même). Contrairement
/// au Makefile, ce repli doit être autonome ici : ce code tourne à l'intérieur du
/// binaire `ocara` déjà compilé, longtemps après que le PKG_CONFIG_PATH du build
/// (`.pkgconfig-shim/`) ait cessé d'exister.
fn pkg_config_libs(package: &str) -> Vec<String> {
    let try_pkg = |pkg: &str| -> Option<Vec<String>> {
        let out = Command::new("pkg-config").arg("--libs").arg(pkg).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        Some(s.split_whitespace().map(|f| f.to_string()).collect())
    };

    try_pkg(package)
        .or_else(|| {
            package.strip_suffix("-4.0").and_then(|base| try_pkg(&format!("{base}-4.1")))
        })
        .unwrap_or_default()
}

/// Écrit les bytes objet dans `obj_path` puis lance le linker système.
///
/// `needs_tauri` : vrai si le programme compilé importe `ocara.Tauri` (déterminé
/// par l'appelant via `ir_module.imports`). Contrôle si libocara_runtime_tauri.a
/// et les bibliothèques GTK/WebKit sont liés — voir la doc de RUNTIME_TAURI_BYTES.
/// `needs_sdl` : même principe pour `ocara.SDL` — voir la doc de RUNTIME_SDL_BYTES.
pub fn link(
    obj_bytes:   &[u8],
    obj_path:    &Path,
    out_path:    &Path,
    release:     bool,
    needs_tauri: bool,
    needs_sdl:   bool,
) -> Result<(), LinkerError> {
    // 1. Écriture du fichier objet
    std::fs::write(obj_path, obj_bytes)
        .map_err(|e| LinkerError(format!("écriture objet: {}", e)))?;

    // 2. Extraction du/des runtime(s) embarqué(s) dans /tmp
    let runtime = extract_to_tmp(RUNTIME_BYTES, "ocara_runtime")?;
    let runtime_tauri = if needs_tauri {
        Some(extract_to_tmp(RUNTIME_TAURI_BYTES, "ocara_runtime_tauri")?)
    } else {
        None
    };
    let runtime_sdl = if needs_sdl {
        Some(extract_to_tmp(RUNTIME_SDL_BYTES, "ocara_runtime_sdl")?)
    } else {
        None
    };

    // 3. Liaison : objet + runtime(s) → exécutable
    // --allow-multiple-definition : les symboles du .o (programme) priment sur la .a (runtime)
    let mut cmd = Command::new("cc");
    cmd.arg(obj_path)
        .arg(&runtime);
    if let Some(rt) = &runtime_tauri {
        cmd.arg(rt);
    }
    if let Some(rt) = &runtime_sdl {
        cmd.arg(rt);
    }
    cmd.arg("-o")
        .arg(out_path)
        .arg("-lm")
        // Pas de -lssl/-lcrypto/-lz : OpenSSL (runtime/Cargo.toml, feature
        // "vendored") et zlib (libz-sys, feature "static") sont compilés depuis
        // les sources et intégrés statiquement à libocara_runtime.a — aucun
        // programme compilé avec Ocara n'exige plus libssl.so/libcrypto.so/libz.so
        // sur la machine cible. Les passer ici serait non seulement inutile mais
        // ferait échouer le lien sur une machine de build sans libssl-dev/zlib1g-dev.
        .arg("-no-pie")
        .arg("-Wl,--allow-multiple-definition")
        // --gc-sections/--as-needed : élague au lien tout ce qui n'est pas
        // réellement atteignable depuis le programme (rustc émet une section ELF
        // par fonction par défaut). Redondant avec la séparation de crate pour
        // Tauri (déjà exclu du lien si needs_tauri est faux), mais garde le même
        // bénéfice pour les autres builtins volumineux (SQLite, MySQL, regex...).
        .arg("-Wl,--gc-sections")
        .arg("-Wl,--as-needed");

    // GTK/WebKit : uniquement si le programme importe réellement ocara.Tauri.
    if needs_tauri {
        for pkg in ["gtk+-3.0", "webkit2gtk-4.0", "javascriptcoregtk-4.0"] {
            for flag in pkg_config_libs(pkg) {
                cmd.arg(flag);
            }
        }
    }

    // --release : demande au linker de supprimer les symboles (strip intégré)
    if release {
        cmd.arg("-Wl,-s");
    }

    let status = cmd.status()
        .map_err(|e| LinkerError(format!("impossible de lancer cc: {}", e)))?;

    // 4. Nettoyage des fichiers temporaires
    let _ = std::fs::remove_file(&runtime);
    if let Some(rt) = &runtime_tauri {
        let _ = std::fs::remove_file(rt);
    }
    if let Some(rt) = &runtime_sdl {
        let _ = std::fs::remove_file(rt);
    }

    if !status.success() {
        return Err(LinkerError(format!(
            "cc a échoué avec le code: {:?}",
            status.code()
        )));
    }

    Ok(())
}
