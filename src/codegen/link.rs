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

/// Runtime embarqué dans le binaire du compilateur (via build.rs + include_bytes!).
static RUNTIME_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libocara_runtime.a"));

/// Extrait le runtime embarqué dans un fichier temporaire et retourne son chemin.
fn extract_runtime() -> Result<PathBuf, LinkerError> {
    let path = std::env::temp_dir()
        .join(format!("libocara_runtime_{}.a", std::process::id()));
    std::fs::write(&path, RUNTIME_BYTES)
        .map_err(|e| LinkerError(format!("extraction du runtime: {}", e)))?;
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
pub fn link(
    obj_bytes: &[u8],
    obj_path:  &Path,
    out_path:  &Path,
    release:   bool,
) -> Result<(), LinkerError> {
    // 1. Écriture du fichier objet
    std::fs::write(obj_path, obj_bytes)
        .map_err(|e| LinkerError(format!("écriture objet: {}", e)))?;

    // 2. Extraction du runtime embarqué dans /tmp
    let runtime = extract_runtime()?;

    // 3. Liaison : objet + runtime → exécutable
    // --allow-multiple-definition : les symboles du .o (programme) priment sur la .a (runtime)
    let mut cmd = Command::new("cc");
    cmd.arg(obj_path)
        .arg(&runtime)
        .arg("-o")
        .arg(out_path)
        .arg("-lm")
        .arg("-lssl")
        .arg("-lcrypto")
        .arg("-lz")
        .arg("-no-pie")
        .arg("-Wl,--allow-multiple-definition");

    // GTK/WebKit (builtin Tauri, v2 — libsoup3 uniquement, plus de binding soup2) :
    // libocara_runtime.a embarque toujours ce code (le runtime est compilé une fois
    // pour tous les programmes), donc même un programme qui n'utilise pas Tauri a
    // besoin de ces symboles résolus au lien.
    for pkg in ["gtk+-3.0", "webkit2gtk-4.0", "javascriptcoregtk-4.0"] {
        for flag in pkg_config_libs(pkg) {
            cmd.arg(flag);
        }
    }

    // --release : demande au linker de supprimer les symboles (strip intégré)
    if release {
        cmd.arg("-Wl,-s");
    }

    let status = cmd.status()
        .map_err(|e| LinkerError(format!("impossible de lancer cc: {}", e)))?;

    // 4. Nettoyage du fichier temporaire
    let _ = std::fs::remove_file(&runtime);

    if !status.success() {
        return Err(LinkerError(format!(
            "cc a échoué avec le code: {:?}",
            status.code()
        )));
    }

    Ok(())
}
