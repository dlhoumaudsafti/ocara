use std::path::{Path, PathBuf};
use std::process::Command;

/// Répertoire `target/release/` réellement utilisé par CE build précis —
/// `target/release/` directement pour une compilation normale (host), mais
/// `target/<triple>/release/` dès que `--target` est passé explicitement
/// (même si `<triple>` == l'hôte : Cargo crée quand même ce sous-dossier dans
/// ce cas précis, jamais couvert ici, sans conséquence réelle puisque cette
/// situation ne se produit jamais dans ce projet). Cargo fournit toujours
/// `TARGET`/`HOST` à un build script (voir la référence Cargo,
/// "Environment variables Cargo sets for build scripts") — les comparer est
/// l'heuristique standard pour distinguer les deux cas depuis un build.rs,
/// qui ne reçoit sinon aucun signal direct indiquant si `--target` a été
/// passé. SANS ceci, cross-compiler `ocara` lui-même (`--target
/// x86_64-pc-windows-gnu`, voir docs/roadmap.d/packaging-windows.md)
/// embarquerait silencieusement le `libocara_runtime.a` de l'HÔTE (ELF Linux)
/// au lieu de celui cross-compilé pour la cible (PE Windows) — un binaire
/// `ocara.exe` produit ainsi échouerait au lien (formats d'objet
/// incompatibles), ou pire, silencieusement si jamais toléré par le linker.
fn target_release_dir(manifest_dir: &Path) -> PathBuf {
    let target = std::env::var("TARGET").expect("TARGET non fourni par Cargo à ce build script");
    let host = std::env::var("HOST").expect("HOST non fourni par Cargo à ce build script");
    if target == host {
        manifest_dir.join("target").join("release")
    } else {
        manifest_dir.join("target").join(&target).join("release")
    }
}

/// Compile `crate_name` (`cargo build --release -p <crate_name>`, avec
/// `--target` si CE build est lui-même cross-compilé — voir
/// `target_release_dir`) si son `.a` n'existe pas encore — permet à `cargo
/// build -p ocara` de fonctionner seul, sans passer par `make build` au
/// préalable (voir docs/roadmap.d/packaging-build-cargo.md). `extra_env` :
/// variables d'environnement additionnelles pour cet appel précis (ex:
/// `PKG_CONFIG_PATH` pour `ocara_runtime_tauri`, voir `ensure_pkgconfig_shim`).
fn ensure_runtime_lib(manifest_dir: &Path, crate_name: &str, extra_env: &[(&str, String)]) -> PathBuf {
    let lib_path = target_release_dir(manifest_dir)
        .join(format!("lib{}.a", crate_name));

    if !lib_path.exists() {
        eprintln!(
            "cargo:warning={} introuvable, compilation via `cargo build --release -p {}`...",
            lib_path.display(), crate_name
        );
        let target = std::env::var("TARGET").unwrap();
        let host = std::env::var("HOST").unwrap();
        let mut cmd = Command::new(env!("CARGO"));
        cmd.args(["build", "--release", "-p", crate_name])
            .current_dir(manifest_dir);
        if target != host {
            cmd.args(["--target", &target]);
        }
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

/// Écrit une archive `ar` valide mais VIDE (aucun membre) — juste l'en-tête
/// magique `!<arch>\n` du format, sans aucun objet à l'intérieur. `ocara`
/// embarque `libocara_runtime_tauri.a`/`libocara_runtime_sdl.a` via
/// `include_bytes!` de façon INCONDITIONNELLE (`src/codegen/link.rs`) — ce
/// fichier doit donc exister, même sur une cible où le vrai runtime
/// correspondant n'est pas (encore) disponible (voir son usage dans `main()`
/// ci-dessous). Conséquence assumée : un programme qui importerait
/// `ocara.Tauri`/`ocara.SDL` et serait compilé PAR un `ocara` construit ainsi
/// échouerait au lien final (symboles `Tauri_*`/`SDL_*` non résolus) — un
/// échec de lien honnête et explicite, jamais un binaire silencieusement cassé.
fn write_empty_archive(path: &Path) {
    std::fs::write(path, b"!<arch>\n")
        .unwrap_or_else(|e| panic!("impossible d'écrire l'archive vide {}: {}", path.display(), e));
}

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let target = std::env::var("TARGET").unwrap_or_default();

    // libocara_runtime.a produit par `cargo build --release -p ocara_runtime`
    // (auto-déclenché ci-dessous si absent — voir `ensure_runtime_lib`).
    let runtime_src = ensure_runtime_lib(&manifest_dir, "ocara_runtime", &[]);
    let runtime_dst = out_dir.join("libocara_runtime.a");
    std::fs::copy(&runtime_src, &runtime_dst)
        .expect("impossible de copier libocara_runtime.a dans OUT_DIR");

    // `ocara_runtime_tauri` (WebView2 sur Windows exige l'ABI MSVC — l'import
    // lib fournie par Microsoft est un `.lib` au format COFF/MSVC, illisible
    // par `ar`/`ld` GNU — voir docs/roadmap.d/packaging-windows.md) et
    // `ocara_runtime_sdl` (pas encore essayé pour cette cible) ne sont pas
    // encore disponibles pour une cible Windows GNU : un stub vide satisfait
    // `include_bytes!` sans prétendre à un vrai support (voir
    // `write_empty_archive`).
    if target.contains("windows") {
        write_empty_archive(&out_dir.join("libocara_runtime_tauri.a"));
        write_empty_archive(&out_dir.join("libocara_runtime_sdl.a"));
    } else {
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
    }

    // Recompiler le compilateur si l'un des runtimes change
    println!("cargo:rerun-if-changed=target/release/libocara_runtime.a");
    println!("cargo:rerun-if-changed=target/release/libocara_runtime_tauri.a");
    println!("cargo:rerun-if-changed=target/release/libocara_runtime_sdl.a");
}
