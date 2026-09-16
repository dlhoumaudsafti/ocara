# Support Android non établi pour la compilation du compilateur

Aucune preuve que le compilateur `ocara` ait jamais été construit, ni qu'un programme Ocara ait jamais été compilé, pour une cible Android. Le constat initial (aucune infrastructure de cross-compilation, primitives bas niveau non testées contre Bionic, aucun backend Android pour SDL/Tauri, aucun packaging APK) reste globalement vrai, mais une vérification concrète (lecture du code source de Cranelift/target-lexicon déjà vendorisés, et de `src/codegen/link.rs`) a permis d'affiner : certains points redoutés a priori comme bloquants sont en réalité de la plomberie contenue, d'autres restent de vraies inconnues à tester.

## Stratégie de packaging envisagée : gabarit pré-construit + injection (à la Godot)

Idée proposée : suivre le même principe que Godot Engine — séparer ce qui ne change JAMAIS d'un programme Ocara à l'autre (le squelette APK, le runtime précompilé) de ce qui change à CHAQUE programme (le code utilisateur), pour éviter de reconstruire tout le projet Gradle à chaque compilation.

Différence importante avec Godot : chez eux, le contenu injecté est du **bytecode/data** (GDScript + ressources), interprété par un moteur déjà compilé pour la cible. Chez Ocara, le "contenu" est du **code natif** produit par Cranelift — donc pas une injection de data pure, mais une injection de `.so` déjà cross-compilé pour l'ABI cible. Le principe reste valable, juste appliqué à un artefact différent.

Point concret en notre faveur : **SDL a déjà ce patron tout prêt et éprouvé** — un template de projet Android officiel où on dépose son `.so` compilé dans `jniLibs/` (c'est essentiellement ce que Godot utilise historiquement en interne). `ocara_runtime_sdl` existant déjà, réutiliser directement ce template plutôt que d'en construire un à la Godot réduirait le chantier "packaging" proprement dit à presque rien — à condition que les deux sous-chantiers ci-dessous (cross-compilation Cranelift, portage SDL3) soient résolus en amont.

## Sous-chantiers, avec ce qui est vérifié vs ce qui reste à tester

### 1. Cross-compilation Cranelift vers un triplet Android — moins bloquant que prévu, vérifié

- `src/codegen/emit.d/emitter.rs` appelle aujourd'hui `cranelift_native::builder()`, qui sélectionne TOUJOURS l'architecture de la machine qui compile — aucun mécanisme pour choisir un triplet différent. **Confirmé en lisant le code.**
- **Vérifié dans le source vendorisé** (`~/.cargo/registry/.../cranelift-codegen-0.105.4/src/isa/mod.rs`) : `cranelift_codegen::isa::lookup(triple)` sélectionne le backend uniquement sur `triple.architecture` (`Aarch64`, `X86_64`...), **indépendamment de l'OS/environnement** — Android n'exige donc aucun traitement spécial côté sélection d'ISA.
- Le backend AArch64 existe déjà dans `cranelift-codegen` mais est désactivé par défaut : `default = ["std", "unwind", "host-arch", "timing"]` (`Cargo.toml` du crate) n'inclut pas la feature `"arm64"` (qui, elle, existe et gate `isa_builder!(aarch64, (feature = "arm64"), triple)`). L'activer dans le `Cargo.toml` racine d'Ocara suffirait à l'embarquer dans le binaire `ocara` — qui resterait un binaire hôte (x86_64 Linux) émettant du code natif POUR une autre cible, exactement comme les autres outils basés sur Cranelift (ex. wasmtime) le font déjà pour plusieurs cibles depuis un seul binaire.
- **Vérifié** : `target-lexicon` (dépendance de Cranelift) modélise `Environment::Android`/`Androideabi` en natif (`src/triple.rs`) — `"aarch64-linux-android"`/`"armv7-linux-androideabi"` sont des triplets directement valides.
- **Vérifié** : `CallConv::triple_default` (`src/isa/call_conv.rs`) résout la convention d'appel via `triple.default_calling_convention()` — pour Android/aarch64 ça retombe sur SystemV, la même convention qu'ARM64 Linux classique (AAPCS64 standard). Aucune bizarrerie d'ABI propre à Android à gérer côté génération de code.
- **Travail réel restant, concret et borné** : ajouter un flag `--target` (ou équivalent) côté CLI (`src/core/cli.rs`), router `emitter.rs` vers `isa::lookup(triple)` au lieu de `cranelift_native::builder()` quand ce flag est présent, et activer la feature `"arm64"` (et/ou `"x86"` pour les ABI x86/x86_64 de l'émulateur Android) sur la dépendance `cranelift-codegen` du `Cargo.toml` racine.

### 2. Lien final vers un `.so` Android — vraie réécriture, pas juste une option en plus

**Vérifié en lisant `src/codegen/link.rs`** : le lien est câblé en dur autour d'un seul hôte —
- Un seul `libocara_runtime.a` (et `libocara_runtime_tauri.a`/`libocara_runtime_sdl.a`) est embarqué dans le binaire `ocara` lui-même via `build.rs` + `include_bytes!`, compilé UNE FOIS pour la machine qui construit `ocara`.
- Le lien final invoque `Command::new("cc")` (le compilateur C de la machine hôte) avec `-no-pie`, produisant un exécutable natif Linux classique.

Pour Android, il faudrait :
- Plusieurs jeux de `.a` pré-construits (un par ABI cible : `arm64-v8a`, `armeabi-v7a`, `x86_64`, `x86`), embarqués ou récupérés à la demande — pas un seul comme aujourd'hui.
- Invoquer le clang du NDK (`aarch64-linux-android21-clang` ou équivalent selon l'API level visé) au lieu de `cc` de la machine hôte, avec le sysroot Bionic du NDK.
- Produire un `.so` partagé (`-shared -fPIC`), pas un exécutable `-no-pie` — Android charge le code natif via JNI dans une bibliothèque partagée, jamais comme processus autonome.

C'est une vraie réécriture de `link()`, pas l'ajout d'un simple flag — mais un périmètre clair et contenu (un seul fichier concerné).

### 3. Portage du runtime Rust vers Bionic — probablement direct, pas encore testé

- `runtime/src/mutex.rs` s'appuie sur `libc::pthread_mutex_t` — le crate `libc` a des bindings Android officiels et POSIX-compatibles ; a priori sans surprise, mais jamais compilé ni testé contre Bionic en pratique.
- Android est une cible Tier 2 de rustc (`aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`, `i686-linux-android`), installable via `rustup target add` + le NDK — un chemin standard et déjà emprunté par énormément de projets Rust mobiles.
- Les dépendances C vendorisées du runtime (OpenSSL "vendored", SQLite, zlib "static" — voir `docs/roadmap.d/packaging-build-cargo.md`) sont couramment cross-compilées vers Android par d'autres projets Rust (mécanisme bien rodé, pas propre à Ocara) — probable mais non vérifié ICI spécifiquement pour notre configuration exacte de features.

### 4. Backend Android pour SDL3 — la plus grosse inconnue restante

- SDL3 en amont (le vrai projet C) supporte Android nativement depuis longtemps.
- `ocara_runtime_sdl` utilise le crate Rust `sdl3` (feature `build-from-source-static`, qui compile SDL3 depuis les sources via cmake). **Non vérifié** : est-ce que le build.rs de `sdl3-sys` sait déjà passer le bon fichier de toolchain cmake (`CMAKE_TOOLCHAIN_FILE` du NDK, `ANDROID_ABI`...) pour une cible Android, ou faudrait-il l'adapter/contourner nous-mêmes ? C'est la seule inconnue de ce chantier qui ne peut pas se trancher par lecture de code — il faudrait un essai réel de cross-compilation.
- `ocara_runtime_tauri` (GTK/WebKit) n'a, lui, structurellement aucun équivalent Android (GTK ne tourne pas sur Android) — hors de portée de ce chantier, une Activity Java/Kotlin + vue native serait de toute façon un mécanisme entièrement différent, pas une adaptation de Tauri.

## Ampleur

Revu à la baisse par rapport au constat initial pour les points 1 et 3 (plomberie contenue, chemins bien connus) ; les points 2 et 4 restent un vrai travail (réécriture de `link.rs` ; inconnue non tranchable sans essai réel pour SDL3/NDK). Le packaging final (gabarit APK + injection du `.so`) devient, lui, le sous-chantier le plus léger si le template Android officiel de SDL est réutilisé tel quel plutôt que reconstruit — voir la section stratégie ci-dessus. Toujours *(Massif)* dans l'ensemble, mais désormais décomposé en 4 sous-chantiers de tailles très inégales plutôt qu'un seul bloc opaque.

## Fichiers clés

`src/codegen/emit.d/emitter.rs` (sélection de cible), `src/codegen/link.rs` (lien final), `src/core/cli.rs` (flag `--target` à ajouter), `Cargo.toml` racine (feature `arm64` de `cranelift-codegen`), `runtime/src/mutex.rs`, `runtime_sdl/` (`Cargo.toml`, dépendance `sdl3`/`sdl3-sys`), `runtime_tauri/`, `Makefile`/`build.rs`.
