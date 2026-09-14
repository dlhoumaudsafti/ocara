use std::path::{Path, PathBuf};
use std::process::Command;

/// Compile `crate_name` (`cargo build --release -p <crate_name>`) si son
/// `.a` n'existe pas encore dans `target/release/` — permet à `cargo build -p
/// ocara` de fonctionner seul, sans passer par `make build` au préalable
/// (voir docs/roadmap.d/packaging-build-cargo.md). `extra_env` : variables
/// d'environnement additionnelles pour cet appel précis (ex: `PKG_CONFIG_PATH`
/// pour `ocara_runtime_tauri`, voir `ensure_pkgconfig_shim`).
fn ensure_runtime_lib(manifest_dir: &Path, crate_name: &str, extra_env: &[(&str, String)]) -> PathBuf {
    let lib_path = manifest_dir
        .join("target")
        .join("release")
        .join(format!("lib{}.a", crate_name));

    if !lib_path.exists() {
        eprintln!(
            "cargo:warning={} introuvable, compilation via `cargo build --release -p {}`...",
            lib_path.display(), crate_name
        );
        let mut cmd = Command::new(env!("CARGO"));
        cmd.args(["build", "--release", "-p", crate_name])
            .current_dir(manifest_dir);
        for (key, val) in extra_env {
            cmd.env(key, val);
        }
        let status = cmd.status().unwrap_or_else(|e| {
            panic!("impossible de lancer `cargo build --release -p {}` : {}", crate_name, e)
        });
        if !status.success() {
            panic!(
                "\n\néchec de la compilation automatique de '{}' (voir les erreurs cargo ci-dessus).\n\
                 Essayez `make build` directement pour un message d'erreur plus détaillé\n\
                 (dépendances système manquantes — voir README).\n",
                crate_name
            );
        }
        if !lib_path.exists() {
            panic!(
                "\n\n`cargo build --release -p {}` a réussi mais {} reste introuvable —\n\
                 incohérence inattendue entre le nom du crate et son artefact produit.\n",
                crate_name, lib_path.display()
            );
        }
    }

    lib_path
}

/// Réplique `make pkgconfig-shim` (voir Makefile) : sur les distributions où
/// seuls `webkit2gtk-4.1`/`javascriptcoregtk-4.1` sont installés (Ubuntu
/// ≥ 24.04 et dérivés récents), `ocara_runtime_tauri` a besoin d'un
/// `webkit2gtk-4.0.pc`/`javascriptcoregtk-4.0.pc` — générés ici comme
/// symlinks locaux vers les `.pc` 4.1 (jamais dans le système), uniquement
/// si `pkg-config <pkg>-4.0` échoue ET `pkg-config <pkg>-4.1` réussit.
/// Ne fait rien (no-op, silencieux) sur une distribution où `-4.0` existe
/// déjà, ou si `pkg-config` lui-même n'est pas installé.
fn ensure_pkgconfig_shim(manifest_dir: &Path) -> PathBuf {
    let shim_dir = manifest_dir.join(".pkgconfig-shim");
    let _ = std::fs::create_dir_all(&shim_dir);

    let pkg_config_exists = |pkg: &str| -> bool {
        Command::new("pkg-config").args(["--exists", pkg]).status()
            .map(|s| s.success()).unwrap_or(false)
    };
    let pkg_config_var = |pkg: &str, var: &str| -> Option<String> {
        Command::new("pkg-config").args([format!("--variable={}", var), pkg.to_string()]).output().ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };

    for pkg in ["webkit2gtk", "javascriptcoregtk"] {
        let v40 = format!("{}-4.0", pkg);
        let v41 = format!("{}-4.1", pkg);
        if !pkg_config_exists(&v40) && pkg_config_exists(&v41) {
            if let Some(pcfiledir) = pkg_config_var(&v41, "pcfiledir") {
                let src = PathBuf::from(&pcfiledir).join(format!("{}.pc", v41));
                let dst = shim_dir.join(format!("{}.pc", v40));
                let _ = std::fs::remove_file(&dst); // symlink_file échoue si déjà présent
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(&src, &dst);
                eprintln!("cargo:warning=shim pkg-config: {}.pc -> {}", v40, src.display());
            }
        }
    }

    shim_dir
}

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // libocara_runtime.a produit par `cargo build --release -p ocara_runtime`
    // (auto-déclenché ci-dessous si absent — voir `ensure_runtime_lib`).
    let runtime_src = ensure_runtime_lib(&manifest_dir, "ocara_runtime", &[]);
    let runtime_dst = out_dir.join("libocara_runtime.a");
    std::fs::copy(&runtime_src, &runtime_dst)
        .expect("impossible de copier libocara_runtime.a dans OUT_DIR");

    // libocara_runtime_tauri.a — crate séparé (voir sa doc et src/codegen/link.rs) :
    // extrait/lié SEULEMENT pour les programmes qui importent réellement
    // ocara.Tauri, afin qu'un programme qui ne l'utilise pas n'exige pas GTK/
    // WebKit installés sur la machine cible. Dépend de pkg-config GTK/WebKit
    // à la COMPILATION du compilateur (voir ensure_pkgconfig_shim).
    let pkgconfig_shim = ensure_pkgconfig_shim(&manifest_dir);
    let existing_pkg_config_path = std::env::var("PKG_CONFIG_PATH").unwrap_or_default();
    let tauri_pkg_config_path = format!("{}:{}", pkgconfig_shim.display(), existing_pkg_config_path);
    let tauri_src = ensure_runtime_lib(
        &manifest_dir, "ocara_runtime_tauri",
        &[("PKG_CONFIG_PATH", tauri_pkg_config_path)],
    );
    let tauri_dst = out_dir.join("libocara_runtime_tauri.a");
    std::fs::copy(&tauri_src, &tauri_dst)
        .expect("impossible de copier libocara_runtime_tauri.a dans OUT_DIR");

    // libocara_runtime_sdl.a — crate séparé (voir sa doc et src/codegen/link.rs) :
    // extrait/lié SEULEMENT pour les programmes qui importent réellement
    // ocara.SDL, afin qu'un programme qui ne l'utilise pas n'exige pas SDL3
    // installé sur la machine cible (SDL3 est lié statiquement — voir
    // runtime_sdl/Cargo.toml — donc en réalité aucun programme compilé n'a
    // besoin de SDL3 sur la machine cible, même ceux qui l'importent).
    // Nécessite cmake + un compilateur C (+ headers X11 dev sur Linux) : si
    // absents, `ensure_runtime_lib` panique avec l'erreur cargo d'origine.
    let sdl_src = ensure_runtime_lib(&manifest_dir, "ocara_runtime_sdl", &[]);
    let sdl_dst = out_dir.join("libocara_runtime_sdl.a");
    std::fs::copy(&sdl_src, &sdl_dst)
        .expect("impossible de copier libocara_runtime_sdl.a dans OUT_DIR");

    // Recompiler le compilateur si l'un des runtimes change
    println!("cargo:rerun-if-changed=target/release/libocara_runtime.a");
    println!("cargo:rerun-if-changed=target/release/libocara_runtime_tauri.a");
    println!("cargo:rerun-if-changed=target/release/libocara_runtime_sdl.a");
}
