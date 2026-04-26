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

    // Recompiler le compilateur si le runtime change
    println!("cargo:rerun-if-changed=target/release/libocara_runtime.a");
}
