# Le compilateur n'est pas "cargo-buildable" nativement

## Constat

`build.rs` (racine) exige que `libocara_runtime.a`, `libocara_runtime_tauri.a` et `libocara_runtime_sdl.a` existent déjà dans `target/release/` avant de pouvoir compiler le crate `ocara` — sinon `panic!` explicite (build.rs:13-19,34-40,57-63). Un simple `cargo build -p ocara` échoue sans être passé au préalable par la séquence orchestrée du `Makefile` (`make build` compile les 3 sous-crates dans l'ordre puis `ocara`).

Deux détails aggravants :

- `build.rs:70-72` lie inconditionnellement OpenSSL au binaire compilateur, même pour un usage qui n'utilisera jamais MySQL.
- `Makefile` impose `-j1` pour compiler `ocara` (pas les runtimes) car Cranelift sature la mémoire en parallèle (SIGKILL) — contrainte documentée mais non résolue.

Un contournement ad hoc apparenté : `.pkgconfig-shim/` (workaround pour `webkit2gtk-4.0`→`4.1` sur Ubuntu ≥ 24.04, génère des symlinks `.pc` locaux) — fonctionne mais reste un patch spécifique Ubuntu/Debian récents plutôt qu'une détection robuste et généralisée.

## Impact

Bloque tout packaging standard (`cargo install`, publication sur crates.io, build reproductible générique) : il faut connaître et respecter la logique du Makefile plutôt que la commande Cargo usuelle.

## Ampleur

Moyen à gros : soit un `build.rs` qui invoque lui-même la compilation des sous-crates runtime, soit une fusion en dépendances Cargo normales avec ordre de build déclaré nativement. Le nettoyage du shim pkg-config et le retrait du lien OpenSSL inconditionnel sont, eux, des correctifs légers et isolés.

## Fichiers clés

`build.rs`, `Makefile`, `.pkgconfig-shim/`.
