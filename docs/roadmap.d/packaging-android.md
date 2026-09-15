# Support Android non établi pour la compilation du compilateur

Aucune preuve que le compilateur `ocara` ait jamais été construit, ni qu'un programme Ocara ait jamais été compilé, pour une cible Android :

- **Aucune infrastructure de cross-compilation du tout aujourd'hui** : le codegen sélectionne toujours la cible native de la machine qui compile (`cranelift_native::builder()`, `src/codegen/emit.d/emitter.rs`) — il n'existe aucun mécanisme pour choisir un triplet cible différent (`aarch64-linux-android`, `armv7-linux-androideabi`, ...), donc même produire un binaire ELF pour Android depuis Linux/macOS n'est pas possible en l'état, indépendamment de toute question d'exécution sur le device.
- **`runtime/src/mutex.rs` s'appuie sur `libc::pthread_mutex_t`** — présent sur Bionic (la libc d'Android) mais jamais compilé/testé contre elle.
- **SDL/Tauri** : `ocara_runtime_sdl` lie SDL3 compilé depuis les sources pour la cible native ; `ocara_runtime_tauri` dépend de GTK/WebKit via pkg-config, tous deux absents d'un environnement Android (qui utiliserait normalement une Activity Java/Kotlin + une vue native, pas GTK). Aucun des deux runtimes graphiques n'a d'équivalent Android.
- **Packaging** : Android attend un APK (code natif embarqué via JNI, `libocara_runtime.so`, manifest, signature) — rien de tout ça n'existe dans `Makefile`/`build.rs`, pensés uniquement pour produire un binaire natif exécutable directement.

## Ampleur

Massif, sur plusieurs plans indépendants qui devraient chacun être résolus : cross-compilation Rust/Cranelift vers une cible Android (toolchain NDK, triplet cible), portage des primitives bas niveau (`Mutex`, threads) vers Bionic, un vrai pont d'exécution (JNI ou équivalent) pour héberger le binaire produit dans une app Android, et une histoire distincte pour SDL/Tauri sur cette plateforme (aucun des deux n'a de backend Android aujourd'hui).

## Fichiers clés

`src/codegen/emit.d/emitter.rs` (sélection de cible), `runtime/src/mutex.rs`, `runtime_sdl/`, `runtime_tauri/`, `Makefile`/`build.rs`.
