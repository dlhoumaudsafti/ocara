use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // libocara_runtime.a produit par `cargo build --release -p ocara_runtime`
    let runtime_src = manifest_dir
        .join("target")
        .join("release")
        .join("libocara_runtime.a");

    if !runtime_src.exists() {
        panic!(
            "\n\nlibocara_runtime.a introuvable dans {}\n\
             Utilisez `make build` (ou `cargo build --release -p ocara_runtime` en premier).\n",
            runtime_src.display()
        );
    }

    let runtime_dst = out_dir.join("libocara_runtime.a");
    std::fs::copy(&runtime_src, &runtime_dst)
        .expect("impossible de copier libocara_runtime.a dans OUT_DIR");

    // libocara_runtime_tauri.a — crate séparé (voir sa doc et src/codegen/link.rs) :
    // extrait/lié SEULEMENT pour les programmes qui importent réellement
    // ocara.Tauri, afin qu'un programme qui ne l'utilise pas n'exige pas GTK/
    // WebKit installés sur la machine cible.
    let tauri_src = manifest_dir
        .join("target")
        .join("release")
        .join("libocara_runtime_tauri.a");

    if !tauri_src.exists() {
        panic!(
            "\n\nlibocara_runtime_tauri.a introuvable dans {}\n\
             Utilisez `make build` (ou `cargo build --release -p ocara_runtime_tauri` en premier).\n",
            tauri_src.display()
        );
    }

    let tauri_dst = out_dir.join("libocara_runtime_tauri.a");
    std::fs::copy(&tauri_src, &tauri_dst)
        .expect("impossible de copier libocara_runtime_tauri.a dans OUT_DIR");

    // libocara_runtime_sdl.a — crate séparé (voir sa doc et src/codegen/link.rs) :
    // extrait/lié SEULEMENT pour les programmes qui importent réellement
    // ocara.SDL, afin qu'un programme qui ne l'utilise pas n'exige pas SDL3
    // installé sur la machine cible (SDL3 est lié statiquement — voir
    // runtime_sdl/Cargo.toml — donc en réalité aucun programme compilé n'a
    // besoin de SDL3 sur la machine cible, même ceux qui l'importent).
    let sdl_src = manifest_dir
        .join("target")
        .join("release")
        .join("libocara_runtime_sdl.a");

    if !sdl_src.exists() {
        panic!(
            "\n\nlibocara_runtime_sdl.a introuvable dans {}\n\
             Utilisez `make build` (ou `cargo build --release -p ocara_runtime_sdl` en premier).\n\
             Nécessite cmake + un compilateur C (+ headers X11 dev sur Linux) — voir README.\n",
            sdl_src.display()
        );
    }

    let sdl_dst = out_dir.join("libocara_runtime_sdl.a");
    std::fs::copy(&sdl_src, &sdl_dst)
        .expect("impossible de copier libocara_runtime_sdl.a dans OUT_DIR");

    // Lier les bibliothèques dynamiques nécessaires pour MySQL/OpenSSL
    println!("cargo:rustc-link-lib=ssl");
    println!("cargo:rustc-link-lib=crypto");

    // Recompiler le compilateur si l'un des runtimes change
    println!("cargo:rerun-if-changed=target/release/libocara_runtime.a");
    println!("cargo:rerun-if-changed=target/release/libocara_runtime_tauri.a");
    println!("cargo:rerun-if-changed=target/release/libocara_runtime_sdl.a");
}
