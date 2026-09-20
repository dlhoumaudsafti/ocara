# Support Android — infrastructure de cross-compilation établie (SDL/packaging APK restants)

**Mise à jour** : le constat initial ci-dessous ("aucune preuve", "aucune infrastructure") a motivé l'ouverture de ce ticket, mais ne reflète plus l'état actuel — voir les sous-chantiers plus bas, dont 1/2/3 sont maintenant faits et vérifiés (NDK r30 installé, `.o`/`.a`/`.so` AArch64 réels produits et inspectés). Conservé tel quel comme trace de l'analyse initiale.

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

### 3. Portage du runtime Rust vers Bionic — **Fait, vérifié** (pour `ocara_runtime` seul)

**Terminé** pour le crate `ocara_runtime` de base (pas `ocara_runtime_tauri`, hors périmètre définitif — voir §4 ; pas `ocara_runtime_sdl`, sous-chantier 4, toujours non vérifié).

Réalisé :
- NDK Android installé (r30, stable) pour cet environnement, cible Rust `aarch64-linux-android` ajoutée (`rustup target add`).
- Nouvelle cible `make build-runtime-android` (`Makefile`) : cross-compile `ocara_runtime` avec le clang du NDK comme compilateur C (requis par les dépendances C vendorisées : OpenSSL "vendored", SQLite "bundled", zlib — voir plus bas), niveau d'API 24.
- **Piège rencontré et corrigé** : le NDK ≥ r23 (tout-LLVM) ne fournit plus les wrappers binutils préfixés par triple (`aarch64-linux-android-ranlib`) — seuls `llvm-ranlib`/`llvm-ar` existent. La crate `cc` (utilisée par `openssl-src`) essaie `llvm-ranlib` **sans chemin complet** (résolu via `$PATH`) avant de retomber sur le nom préfixé inexistant, faisant échouer `make install_dev` d'OpenSSL en toute fin de build (`aarch64-linux-android-ranlib: not found`). Corrigé en fixant explicitement `RANLIB_aarch64_linux_android` vers le `llvm-ranlib` du NDK dans la cible Make.
- **Confirmé, PAS supposé** : `libz-sys` (dépendance transitive d'`ureq`/`mysql` via `flate2`) **ignore délibérément** la feature `"static"` sur Android — son propre `build.rs` part du principe que « tout compilateur Android est livré avec libz » et force toujours un lien dynamique (`cargo:rustc-link-lib=z`), contrairement à OpenSSL ("vendored", vraiment statique y compris pour Android — vérifié : ni `libssl.so` ni `libcrypto.so` n'apparaissent dans les `NEEDED` du `.so` final). Ce détail a un impact direct sur le sous-chantier 2 (voir plus bas, `-lz`).

#### Vérification

`libocara_runtime.a` produit pour `aarch64-linux-android` : `ar x` + `readelf -h` sur un objet interne confirme `Machine: AArch64` (vs x86_64 pour le build hôte). Compilation complète d'OpenSSL vendored, SQLite bundled, zlib, et toutes les dépendances pures Rust (serde, regex, tiny_http, mysql...) sans erreur pour cette cible.

**Non couvert** : `armv7-linux-androideabi`/`x86_64-linux-android`/`i686-linux-android` (seul `aarch64` a été testé — de loin l'ABI la plus pertinente, l'écrasante majorité des appareils Android actuels étant en 64 bits) ; exécution réelle du runtime sur Bionic (nécessite un appareil/émulateur, hors de portée de cet environnement).

### 2. Lien final vers un `.so` Android — **Fait, vérifié**

**Terminé.** `src/codegen/link.rs` gagne une fonction séparée, `link_android`, à côté de `link()` (chemin hôte, inchangé) — pas une réécriture du chemin existant, une **addition** :

- Nouveaux flags CLI `--android-runtime <fichier.a>` et `--android-ndk <dir>` (`src/core/cli.rs`). Contrairement au runtime hôte (embarqué dans `ocara` via `include_bytes!` à SA PROPRE compilation), le runtime Android **n'est pas embarqué** : il est fourni à la demande via `--android-runtime` (typiquement le `.a` produit par `make build-runtime-android`, §3 ci-dessus) — `ocara` reste un binaire hôte ordinaire, buildable sans le NDK, exactement le choix "récupéré à la demande" envisagé dans la stratégie de packaging ci-dessus plutôt qu'"embarqué".
- `link_android` résout le clang du NDK à partir du triple cible (`aarch64-linux-android` → `aarch64-linux-android24-clang`, niveau d'API fixé à 24 — DOIT correspondre à celui utilisé pour compiler le runtime, voir §3) et invoque `<clang> obj.o runtime.a -o out.so -shared -fPIC -lm -lz ...` — pas de `-no-pie` (propre au chemin exécutable hôte), pas de pkg-config/GTK (jamais pertinent pour Android).
- **Découverte critique, pas anticipée par la version précédente de ce ticket** : le dynamic linker Bionic d'Android **refuse catégoriquement** de charger un `.so` avec un segment `DT_TEXTREL` — contrairement à un exécutable `-no-pie` glibc, qui tolère l'équivalent (voir l'audit dans [securite-lien-no-pie](securite-lien-no-pie.md)). Comme Cranelift n'active jamais `is_pic` par défaut (`settings::Flags::new`, réglage historique d'Ocara), le `.o` généré pour Android aurait produit exactement ce `DT_TEXTREL` interdit. Corrigé dans `CraneliftEmitter::new` (`emit.d/emitter.rs`) : `is_pic=true` est désormais activé **quand un `--target` explicite est fourni** (cross-compilation), et UNIQUEMENT dans ce cas — le chemin hôte par défaut reste inchangé (`is_pic=false`, `-no-pie` toujours en place, voir la note ajoutée à [securite-pie-cranelift-is-pic](securite-pie-cranelift-is-pic.md) qui documente cette découverte sans rouvrir ce ticket-là, resté hors périmètre).
- **Découverte secondaire** : `-lz` manquant à la première tentative — `deflate`/`inflate`/`zlibVersion` restaient non résolus (ni `weak`, ni rattachés à aucune bibliothèque `NEEDED`) dans le `.so` produit, à cause du comportement de `libz-sys` documenté en §3. Le NDK fournit bien un `libz.so` de liaison pour toutes les API levels — `-lz` ajouté, corrigé.
- Ni Tauri ni SDL ne sont supportés par `link_android` : les deux sont rejetés explicitement AVANT la liaison (`main.rs`, `ir_module.imports`) si le programme les importe, plutôt que de produire un `.so` silencieusement incomplet — la même famille de bug que cette session a passé énormément de temps à corriger côté codegen (symbole/appel silencieusement ignoré, voir [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md)).

#### Vérification

`ocara main.oc --target aarch64-linux-android --android-runtime libocara_runtime.a --android-ndk <NDK> -o libmain.so` produit un `.so` inspecté avec `readelf`/`nm`, sans NDK côté exécution (aucun appareil/émulateur requis pour ces vérifications) :

```
$ file libmain.so
libmain.so: ELF 64-bit LSB shared object, ARM aarch64, ..., dynamically linked, ...
$ readelf -h libmain.so | grep Type
  Type:                              DYN (fichier objet partagé)
$ readelf -d libmain.so | grep -i textrel
(rien — pas de DT_TEXTREL, le point critique pour Bionic)
$ readelf -d libmain.so | grep NEEDED
 NEEDED  libm.so
 NEEDED  libz.so
 NEEDED  libdl.so
 NEEDED  libc.so
$ nm -D --undefined-only libmain.so | grep -v '@LIBC' | awk '$1!="w"'
(vide — tout symbole non-libc non-weak restant est couvert par une NEEDED : ici, zlib)
```

Les symboles `w` (weak, ex: `getrandom`, `copy_file_range`, hooks de trace `ZSTD_*`) sont un mécanisme normal de détection de fonctionnalité à l'exécution (résolus à `NULL` si absents, vérifiés par l'appelant) — pas des dépendances manquantes.

`make build` : 0 warning. `make tests` : 101 passed. `make regression` (cache vidé) : 684 + 50 PASS, 0 FAIL — le chemin hôte (`is_pic=false`, `link()` inchangé) n'est pas affecté par l'ajout de `link_android`.

**Non couvert, seule chose qui reste vraiment inconnue** : le comportement du `.so` **au chargement et à l'exécution réels** sur un appareil/émulateur Android (JNI, `System.loadLibrary`, appel effectif de `main`) — nécessite un appareil ou un émulateur configuré, hors de portée de cet environnement. Tout ce qui est vérifiable par inspection statique du binaire (ELF valide, PIC, pas de TEXTREL, toutes les dépendances dynamiques déclarées et réellement exportées par les bibliothèques système du NDK) l'a été.

### 4. Backend Android pour SDL3 — la plus grosse inconnue restante

- SDL3 en amont (le vrai projet C) supporte Android nativement depuis longtemps.
- `ocara_runtime_sdl` utilise le crate Rust `sdl3` (feature `build-from-source-static`, qui compile SDL3 depuis les sources via cmake). **Non vérifié** : est-ce que le build.rs de `sdl3-sys` sait déjà passer le bon fichier de toolchain cmake (`CMAKE_TOOLCHAIN_FILE` du NDK, `ANDROID_ABI`...) pour une cible Android, ou faudrait-il l'adapter/contourner nous-mêmes ? C'est la seule inconnue de ce chantier qui ne peut pas se trancher par lecture de code — il faudrait un essai réel de cross-compilation.
- `ocara_runtime_tauri` (GTK/WebKit) n'a, lui, structurellement aucun équivalent Android (GTK ne tourne pas sur Android) — hors de portée de ce chantier, une Activity Java/Kotlin + vue native serait de toute façon un mécanisme entièrement différent, pas une adaptation de Tauri.

## Ampleur

Sous-chantiers **1, 2 et 3 faits et vérifiés** (NDK r30 installé dans cet environnement). Il ne reste que le sous-chantier 4 (backend Android pour SDL3 — la vraie inconnue non tranchable sans essai réel de cross-compilation cmake/NDK) et, une fois celui-ci résolu, le packaging final proprement dit (gabarit APK + injection du `.so`, potentiellement léger si le template Android officiel de SDL est réutilisé tel quel — voir la section stratégie ci-dessus). Ce qui reste non vérifiable dans cet environnement, quel que soit l'état du code : le comportement réel du `.so` produit une fois chargé sur un appareil/émulateur Android (JNI, `System.loadLibrary`) — nécessite un device réel ou un émulateur configuré. Toujours *(Massif)* en ampleur globale (SDL3/NDK + packaging APK restent un vrai travail), mais 3 sous-chantiers sur 4 sont désormais clos.

## Fichiers clés

`src/codegen/emit.d/emitter.rs` (sélection de cible + `is_pic` pour cible croisée — **fait**), `src/core/cli.rs` (flags `--target`/`--android-runtime`/`--android-ndk` — **fait**), `Cargo.toml` racine (feature `arm64` de `cranelift-codegen` — **fait**), `src/codegen/link.rs` (`link_android`, lien final Android — **fait**), `Makefile` (cible `build-runtime-android` — **fait**), `runtime/src/mutex.rs` (compilé et vérifié pour Bionic — **fait**), `runtime_sdl/` (`Cargo.toml`, dépendance `sdl3`/`sdl3-sys` — sous-chantier 4, pas fait), `runtime_tauri/` (hors périmètre Android, définitif), [securite-pie-cranelift-is-pic](securite-pie-cranelift-is-pic.md) (ticket voisin, note ajoutée sur `is_pic`, resté non traité pour le chemin hôte).
