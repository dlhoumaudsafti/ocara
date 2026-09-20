# Support Android non établi pour la compilation du compilateur

Aucune preuve que le compilateur `ocara` ait jamais été construit, ni qu'un programme Ocara ait jamais été compilé, pour une cible Android. Le constat initial (aucune infrastructure de cross-compilation, primitives bas niveau non testées contre Bionic, aucun backend Android pour SDL/Tauri, aucun packaging APK) reste globalement vrai, mais une vérification concrète (lecture du code source de Cranelift/target-lexicon déjà vendorisés, et de `src/codegen/link.rs`) a permis d'affiner : certains points redoutés a priori comme bloquants sont en réalité de la plomberie contenue, d'autres restent de vraies inconnues à tester.

## Stratégie de packaging envisagée : gabarit pré-construit + injection (à la Godot)

Idée proposée : suivre le même principe que Godot Engine — séparer ce qui ne change JAMAIS d'un programme Ocara à l'autre (le squelette APK, le runtime précompilé) de ce qui change à CHAQUE programme (le code utilisateur), pour éviter de reconstruire tout le projet Gradle à chaque compilation.

Différence importante avec Godot : chez eux, le contenu injecté est du **bytecode/data** (GDScript + ressources), interprété par un moteur déjà compilé pour la cible. Chez Ocara, le "contenu" est du **code natif** produit par Cranelift — donc pas une injection de data pure, mais une injection de `.so` déjà cross-compilé pour l'ABI cible. Le principe reste valable, juste appliqué à un artefact différent.

Point concret en notre faveur : **SDL a déjà ce patron tout prêt et éprouvé** — un template de projet Android officiel où on dépose son `.so` compilé dans `jniLibs/` (c'est essentiellement ce que Godot utilise historiquement en interne). `ocara_runtime_sdl` existant déjà, réutiliser directement ce template plutôt que d'en construire un à la Godot réduirait le chantier "packaging" proprement dit à presque rien — à condition que les deux sous-chantiers ci-dessous (cross-compilation Cranelift, portage SDL3) soient résolus en amont.

## Sous-chantiers, avec ce qui est vérifié vs ce qui reste à tester

### 1. Cross-compilation Cranelift vers un triplet Android — **Fait, vérifié**

**Terminé.** `ocara` reste un binaire hôte (x86_64 Linux) mais peut désormais émettre du code objet natif pour une autre architecture, sans jamais être lui-même recompilé pour cette cible.

Réalisé :
- Nouveau flag CLI `--target <triple>` (`src/core/cli.rs`), ex. `--target aarch64-linux-android`.
- `Cargo.toml` racine : la dépendance `cranelift-codegen` déclare désormais `features = ["arm64"]` en plus des features par défaut — le backend AArch64 est embarqué dans le binaire `ocara` (les défauts, via `host-arch`, embarquaient déjà le backend x86 puisque `ocara` lui-même est construit pour x86_64 — voir `cranelift-codegen/build.rs`, qui détecte l'architecture de compilation d'`ocara` via la variable d'env `TARGET` et n'active QUE cette architecture sans la feature `arm64` explicite).
- `src/codegen/emit.d/emitter.rs`, `CraneliftEmitter::new` prend désormais `target: Option<&str>` : si `Some`, parse le triple (`target_lexicon::Triple::from_str`) et route vers `cranelift_codegen::isa::lookup(triple)` ; si `None`, comportement historique inchangé (`cranelift_native::builder()`). Volontairement, **`cranelift_native::infer_native_flags` n'est PAS appelé pour une cible explicite** — cette fonction sonde les extensions CPU de la machine qui exécute `ocara` (via des `#[cfg(target_arch = ...)]` sur l'architecture de compilation d'`ocara`, pas sur la cible visée), l'appliquer à un ISA différent n'aurait aucun sens ; on se contente des `settings::Flags` par défaut (plancher portable, sans extension CPU spécifique).
- `src/main.rs` : `--target` sans `--no-link` est explicitement rejeté à la compilation (message clair renvoyant vers ce ticket) — la liaison finale (`link.rs`) reste câblée pour l'hôte (sous-chantier 2, pas fait), une cible croisée sans ce garde-fou produirait un binaire cassé plutôt qu'une erreur nette.

### Vérification

Avec `--target aarch64-linux-android --no-link`, le `.o` produit est inspecté avec `readelf -h`/`file` (pas de NDK nécessaire — cette étape ne fait QUE de la génération de code objet, jamais de liaison) :

```
$ file hello_android.o
hello_android.o: ELF 64-bit LSB relocatable, ARM aarch64, version 1 (SYSV), not stripped
$ readelf -h hello_android.o | grep Machine
  Machine:                           AArch64
```

À comparer à la compilation par défaut (même fichier source, sans `--target`) :
```
$ file hello_host.o
hello_host.o: ELF 64-bit LSB relocatable, x86-64, version 1 (SYSV), not stripped
```

Les symboles (`main`, `__fn_wrap_main`, ...) sont présents et nommés correctement dans le `.o` ARM64 (`objdump -t`). `--target aarch64-unknown-linux-gnu` (ARM64 Linux non-Android) produit le même résultat, confirmant que le chemin de sélection d'ISA ne dépend bien que de l'architecture, pas de l'environnement (Android ou non) — exactement ce que la lecture de `isa/mod.rs` prédisait. Un triple invalide (`--target not-a-real-triple`) échoue proprement avec un message d'erreur, pas un panic.

`make build` : 0 warning. `make tests` : 101 passed, 0 failed (aucun test existant ne dépendait de la signature précédente de `CraneliftEmitter::new`, seul appelant : `src/main.rs`). `make regression` (cache vidé) : 684 + 50 PASS, 0 FAIL, 0 ERREUR — le chemin de compilation par défaut (sans `--target`, l'écrasante majorité des invocations) n'est pas affecté.

**Non couvert par ce sous-chantier** (attendu, scope volontairement restreint à la génération de code objet) : liaison finale pour une cible croisée (sous-chantier 2, `.so` NDK), exécution réelle du code généré sur un appareil/émulateur Android (nécessite le NDK + sous-chantiers 2-4), ABI x86/x86_64 (émulateur Android) — la feature `"x86"` de `cranelift-codegen` n'a pas été activée, seule `"arm64"` a été ajoutée ; à faire de la même façon si un jour nécessaire (même mécanisme, non testé ici faute de besoin immédiat).

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

Sous-chantier 1 **fait**. Les points 2 et 4 restent un vrai travail (réécriture de `link.rs` ; inconnue non tranchable sans essai réel pour SDL3/NDK) ; le point 3 (portage runtime vers Bionic) reste probable mais non testé — nécessite le NDK, pas encore installé dans cet environnement. Le packaging final (gabarit APK + injection du `.so`) devient, lui, le sous-chantier le plus léger si le template Android officiel de SDL est réutilisé tel quel plutôt que reconstruit — voir la section stratégie ci-dessus. Toujours *(Massif)* dans l'ensemble : 1 sous-chantier sur 4 fait, les 3 restants nécessitent tous le NDK pour être vérifiés (pas encore demandé/installé).

## Fichiers clés

`src/codegen/emit.d/emitter.rs` (sélection de cible — **fait**), `src/core/cli.rs` (flag `--target` — **fait**), `Cargo.toml` racine (feature `arm64` de `cranelift-codegen` — **fait**), `src/codegen/link.rs` (lien final — sous-chantier 2, pas fait), `runtime/src/mutex.rs`, `runtime_sdl/` (`Cargo.toml`, dépendance `sdl3`/`sdl3-sys`), `runtime_tauri/`, `Makefile`/`build.rs`.
