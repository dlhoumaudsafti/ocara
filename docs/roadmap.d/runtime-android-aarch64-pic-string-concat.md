# Concaténation corrompue sur AArch64 réel — cause racine réelle : pointeurs tagués par Scudo (Android), pas du codegen

## Terminé — corrigé et vérifié en exécution réelle sur un vrai appareil

**La première version de ce ticket avançait une hypothèse de codegen Cranelift AArch64+PIC — elle était fausse.** La cause racine réelle, trouvée en lisant réellement le code plutôt qu'en re-devinant, est documentée ci-dessous, ainsi que le correctif et sa vérification complète sur un vrai téléphone (Realme RMX3834, arm64-v8a) : `curl` via `adb forward` répond `HTTP 200, 2306 octets` avec le HTML complet et correct (au lieu d'un nombre corrompu), taille identique à celle obtenue sur x86_64 — plus aucun écart entre desktop, émulateur et appareil réel.

## Constat (inchangé)

Sur un **vrai appareil Android arm64-v8a**, `System::OS + "-world"` (et plus généralement toute concaténation impliquant une chaîne allouée par le runtime — `Convert::intToStr`, `System::OS`, etc.) renvoyait un grand nombre au lieu du texte attendu, ex. `-5476376631301240768-world` au lieu de `android-world`. Jamais reproduit sur x86_64 (émulateur ou hôte), ni avec des chaînes littérales.

## Cause racine réelle : les pointeurs de tas Android portent un tag dans leur octet de poids fort

`ocara_runtime` distingue au runtime un entier brut d'un pointeur de tas **par la magnitude de sa valeur i64** (`is_ptr`/`is_float_box`/`is_bool_box`/`is_int_box` dans `runtime/src/lib.rs`, `get_value_type`/`PTR_THRESHOLD`/`MAX_USERSPACE_ADDR` — et leurs copies dans `runtime/src/typecheck.rs`/`runtime/src/yaml.rs`) : un pointeur valide est censé être positif, `>= 0x10000`, `< 0x800000000000` (2^47, la limite typique d'un espace utilisateur 48 bits sur Linux/x86_64). Cette hypothèse est **vraie sur x86_64**, mais **fausse sur un vrai appareil Android arm64 depuis Android 11** : voir <https://source.android.com/docs/security/test/tagged-pointers> — l'allocateur natif de Bionic (Scudo) peut renvoyer des pointeurs dont l'**octet de poids fort (bits 56-63)** porte un tag non nul, utilisé pour la détection d'use-after-free. Le CPU ARM64 ignore ce tag pour les accès mémoire réels (extension matérielle "Top-Byte-Ignore") — un tel pointeur reste donc parfaitement déréférençable — mais **pas** pour une comparaison numérique i64 ordinaire : avec le tag observé (`0xb4` dans nos reproductions), la valeur devient soit négative soit très supérieure à `0x800000000000`.

Vérifié en décodant les valeurs corrompues observées (`-5476376631301240768` etc.) : leur représentation hexadécimale commence bien par `0xb4...`, cohérent avec un octet de poids fort tagué plutôt qu'une adresse aléatoire. Confirmé une seconde fois directement dans un vrai tombstone (`tagged_addr_ctrl: 0000000000000001 (PR_TAGGED_ADDR_ENABLE)`, registres de threads affichant des valeurs `x0 b4000076fb031b60`) : le tagging est bien actif sur cet appareil.

Conséquence exacte, tracée dans le code : `val_to_string` (`runtime/src/lib.rs`), appelée par `__str_concat` ET par `__dyn_add` (le point d'entrée réel de l'opérateur `+` dès qu'un opérande est `Ptr`/`mixed`, voir `src/lower/expr.d/lower.rs:1213`), teste `is_ptr(val)` (`val >= 0x10000 && (val & 3) == 0`) — pour un pointeur tagué interprété comme négatif, cette comparaison SIGNÉE échoue, et `val_to_string` tombe dans son cas par défaut `val.to_string()` : **elle affiche la représentation décimale brute du pointeur tagué au lieu de déréférencer la chaîne.** Aucun rapport avec Cranelift, `is_pic`, ou l'AArch64 en tant qu'architecture de compilation — uniquement avec le comportement de l'allocateur natif d'Android au runtime.

Pourquoi seulement les chaînes allouées par le runtime (`alloc_str`, donc `malloc`/Scudo) et jamais les littéraux : un littéral vit dans le `.so` lui-même (section de données, adresse basse, jamais taguée par Scudo) — seul un pointeur réellement issu de l'allocateur natif peut porter ce tag.

## Correctif

Plutôt que de réécrire la logique de classification par magnitude (risque réel : un entier négatif brut légitime dans un `mixed` peut avoir n'importe quel octet de poids fort selon sa valeur — masquer le tag sans discriminer finement aurait pu casser cette distinction existante), le tag est supprimé **à la source** : `runtime_android_jni/src/lib.rs`, tout début de `Java_com_ocara_bridge_OcaraBridge_nativeStartServer`, appelle désormais `mallopt(M_BIONIC_SET_HEAP_TAGGING_LEVEL, M_HEAP_TAGGING_LEVEL_NONE)` (constantes `-204`/`0`, vérifiées contre les sources Bionic officielles — `android.googlesource.com/platform/bionic` et son miroir GitHub AOSP) — un réglage **process-wide** qui désactive le tagging pour tout le processus, à partir de cet appel, avant même que le thread lancé pour `main()` ne démarre. Tous les pointeurs alloués ensuite par `alloc()` (donc par `alloc_str`/`box_int_if_needed`/... dans `ocara_runtime`) reviennent avec un octet de poids fort à zéro — la logique de classification existante du runtime n'a besoin d'aucune modification.

`mallopt` n'est pas exposé par le crate `libc` pour la cible `android` (vérifié : seulement glibc/hurd/aix/nto) — déclaré à la main via `extern "C"`, résolu dynamiquement contre `libc.so` de Bionic au chargement du `.so` (un symbole réel de Bionic, pas une invention).

Sans effet sur le chemin desktop (x86_64/glibc ne tague jamais ses pointeurs ainsi) : le correctif n'existe que dans `runtime_android_jni`, un crate strictement Android.

## Fausse alerte rencontrée en vérifiant le correctif (piège méthodologique à retenir)

Le tout premier test sur le vrai appareil, juste après ce correctif, semblait montrer un **nouveau crash** : `make android-simulator` + `monkey` rapportait "New tombstone found" avec un signal SIGSEGV pointant directement dans `nativeStartServer`. Analyse détaillée du tombstone (`adb pull`) → crash réel, `SEGV_ACCERR`, dans l'appel à `env.get_string()` juste après `mallopt`. Hypothèse immédiate : `mallopt` casserait l'état JNI.

**Corrigé/infirmé par une vérification, pas une supposition** : `adb shell ls -la /data/tombstones/` a montré que TOUS les tombstones dataient de la **veille** (22 septembre), aucun d'aujourd'hui — `monkey` les "redécouvre" à chaque lancement (il n'a pas de mémoire d'un run à l'autre) et les rapporte comme "New" même s'ils sont anciens et sans rapport (l'un d'eux était même dans `libsprdfacebeauty.so`, une lib vendor de caméra, aucun rapport avec cette app). Un essai en désactivant temporairement `mallopt` (recompilé, réinstallé) a montré exactement le même comportement de "New tombstone" — la preuve que ce n'était PAS spécifique au correctif. Vérification définitive : `adb shell ps -A | grep ocara` (processus vivant) + `curl` via `adb forward` (répond correctement) — la seule méthode fiable, pas les heuristiques de `monkey`.

**Leçon methodologique** : sur cet appareil, `/data/tombstones/` n'est pas vidé entre deux tests et `monkey` n'a aucune notion de "déjà vu" — toujours croiser avec l'horodatage réel des fichiers (`ls -la`, comparé à `adb shell date`) et/ou vérifier l'état vivant du processus (`ps`/`curl`) avant de conclure à un crash à partir de la seule sortie de `monkey`.

## Vérification finale

- Repro isolé minimal (`System::OS + "-world"`) : correct sur l'hôte (`linux-world`), inchangé.
- `runtime_android_jni` recompilé pour `aarch64-linux-android` ET `x86_64-linux-android` (`make build-jni-bridge-android`) : 0 warning.
- `.so` cross-compilé et lié avec `--android-jni-bridge` : ELF `DYN` AArch64, **aucun `DT_TEXTREL`**, `mallopt` apparaît comme symbole non résolu (`U`) dans la table dynamique, correctement satisfait par `libc.so` (déjà `NEEDED`).
- `make build`/`make tests`/`make regression` (hôte) : inchangés, verts.
- **Sur le vrai appareil (Realme RMX3834, arm64-v8a), APK réel réinstallé** (`main.oc` de `examples/advanced/mini_project`, pas une copie de test) : processus vivant (`ps`), `curl` via `adb forward` répond `HTTP 200, 2306 octets` avec le HTML complet et correct (titre, liste de voitures, pied de page) — **taille identique à celle obtenue sur x86_64**, plus de nombre corrompu. `style.css` répond aussi `HTTP 200, 7836 octets`.

## Priorité / Complexité

Clos. La cause racine réelle était beaucoup plus contenue que l'hypothèse de codegen initialement crainte : un seul appel `mallopt` dans un crate déjà dédié à l'intégration Android, sans toucher au compilateur ni à la logique de classification du runtime.

## Fichiers clés

`runtime_android_jni/src/lib.rs` (`mallopt`/`M_BIONIC_SET_HEAP_TAGGING_LEVEL`, **fait, vérifié en exécution réelle**), `runtime/src/lib.rs` (`is_ptr`/`val_to_string`/`get_value_type`/`PTR_THRESHOLD`/`MAX_USERSPACE_ADDR` — la logique de classification affectée, **non modifiée**, corrigée en amont à la source), `runtime/src/typecheck.rs`/`runtime/src/yaml.rs` (copies locales de la même logique de classification — couvertes par le même correctif puisqu'il agit à l'allocation, pas à la lecture), [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) (contexte du test qui l'a révélé, et le correctif Tauri/Android trouvé juste après), [runtime-android-exit-unsafe](runtime-android-exit-unsafe.md) (le bug précédent, résolu, qui masquait celui-ci).
