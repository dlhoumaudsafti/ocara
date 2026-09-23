# `__ocara_fail` (sortie sur exception non gérée) faisait planter le processus sur Android — testé et corrigé sur un vrai appareil

## Terminé — `__ocara_fail` sûr sur Android + cause racine du déclencheur SQLite résolue

Voir §"Ce qui a été fait" plus bas. Les deux causes (le crash lui-même ET son déclencheur, l'échec `SQLite::open()`) ont finalement été résolues ensemble — voir aussi [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) pour la vérification complète.

## Constat (avant correctif)

Découvert en testant `examples/advanced/mini_project/main_android.oc` sur un **vrai appareil Android** (Realme RMX3834, Android 15/API 35, arm64-v8a — voir [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md)), après que le même code a fonctionné correctement sur un émulateur x86_64 (Android API 36, image AOSP standard).

Sur l'appareil réel, `Database::connect()` → `SQLite::open(...)` échouait et déclenchait `throw_sqlite_exception`. Comme rien ne catch cette exception, elle atteignait `__ocara_fail`, qui appelait `std::process::exit()` — et LÀ, le processus **plantait violemment** (`SIGABRT`) au lieu de terminer proprement, avec deux variantes observées (tombstones réels, pas une supposition) :

```
Fatal signal 6 (SIGABRT) ...
#04 art::Mutex::~Mutex()
#05 __cxa_finalize
#06 exit
#07 std::sys::exit::exit
#08 std::process::exit
#09 __ocara_fail
#10 ocara_runtime::exception::throw_sqlite_exception
#11 SQLite_open
#12 Database_connect
```

et, sur une autre tentative (même point d'origine, destructeur différent) :

```
Fatal signal 6 (SIGABRT) ...
#04 android::uirenderer::CommonPool::~CommonPool()
#05 __cxa_finalize
...
```

**Le point commun, dans les deux cas** : `std::process::exit()` déclenche `__cxa_finalize`, qui exécute les destructeurs de TOUS les objets globaux du processus — y compris ceux d'ART (`art::Mutex`) et de composants système Android déjà chargés dans le même processus (`libhwui.so`). Ces objets n'ont jamais été conçus pour être détruits "en cours de vie" d'une app Android — leur destruction déclenche un état invalide qu'ART/Bionic détectent explicitement et transforment en `abort()` fatal, contrairement à un processus Linux normal où `exit()` est sûr et attendu.

## Ce qui a été fait

### 1. `__ocara_fail` rendu sûr sur Android

`runtime/src/lib.rs:3149` — dans la branche "aucun `try` actif" (avant `std::process::exit(1)`) : `#[cfg(target_os = "android")]` appelle désormais `unsafe { libc::_exit(1); }` (l'appel système BRUT, pas `exit()`) au lieu de `std::process::exit(1)`. `libc::_exit()` saute entièrement `__cxa_finalize`/les destructeurs globaux — le processus termine immédiatement sans jamais toucher aux internes d'ART. Le chemin desktop (`std::process::exit(1)`) reste inchangé (`#[cfg(not(target_os = "android"))]`).

### 2. Cause racine de l'échec `SQLite::open()` — trouvée et résolue

Contrairement à ce que ce ticket supposait initialement ("aucune piste concrète"), la cause a été trouvée en généralisant le mécanisme plutôt qu'en creusant SQLite spécifiquement : voir [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) §"chdir vers `Context.getFilesDir()`" — le pont JNI (`runtime_android_jni`) fait maintenant `chdir()` vers le répertoire retourné par `Context.getFilesDir()` (obtenu et créé côté Java, PAS deviné/construit côté natif) avant de démarrer le programme Ocara. Le chemin SQLite relatif d'origine (`"./app.db"`, jamais modifié) pointe alors vers un répertoire réellement créé et étiqueté SELinux correctement par le framework Android — contrairement au chemin absolu construit à la main dans une itération précédente (`/data/user/0/<package>/app.db`, sans passer par `getFilesDir()`), qui fonctionnait sur l'émulateur mais échouait sur ce téléphone précis (probablement des restrictions OEM/ColorOS plus strictes que l'AOSP stock sur l'écriture directe dans la racine du bac à sable plutôt que dans `files/`).

### 3. Piège technique rencontré en implémentant le pont JNI (`chdir`)

Le tout premier essai (bindings `jni-sys` bruts, accès à la table de fonctions JNI via l'union versionnée générée par `jni-sys-macros`, `(*env).v1_1.GetStringUTFChars`) a introduit un **nouveau** crash (`SIGSEGV, code 2 SEGV_ACCERR`, sur le thread principal, juste après le chargement du `.so`) — vraisemblablement un mauvais calcul d'offset dans cette union versionnée, jamais élucidé précisément. Remplacé par la crate `jni` (wrapper sûr, `JNIEnv::get_string`) plutôt que continuer à deviner l'ABI JNI brute à la main — corrigé, plus aucun crash à ce niveau. Coût accepté : dépendances `combine`/`cesu8` en plus dans `runtime_android_jni` — la correction prime sur la taille du binaire pour du code qui plantait silencieusement sur un appareil réel seulement (jamais reproduit sur l'émulateur, la même classe de risque que tout ce chantier Android).

### Vérification

Sur le vrai appareil (Realme RMX3834), après `pm clear` (état totalement propre, élimine l'hypothèse d'un fichier corrompu) : processus vivant, aucun tombstone, `curl` via `adb forward` répond `HTTP 200`. Confirmé à plusieurs reprises, y compris après reconnexion USB.

## Priorité / Complexité

Clos. Voir [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) pour la suite (un NOUVEAU bug distinct, de codegen AArch64+PIC, découvert APRÈS que ce crash a été résolu — voir [runtime-android-aarch64-pic-string-concat](runtime-android-aarch64-pic-string-concat.md)).

## Fichiers clés

`runtime/src/lib.rs:3149` (`__ocara_fail`, fait), `runtime_android_jni/src/lib.rs` (`chdir` + crate `jni`, fait), `runtime_android_jni/Cargo.toml` (dépendances `jni`+`libc`, fait), `packaging/android/app/src/main/java/com/ocara/bridge/OcaraBridge.kt` (signature `nativeStartServer(dataDir: String)`, fait), `packaging/android/app/src/main/java/com/ocara/demo/MainActivity.kt` (`filesDir.absolutePath`, extraction des assets, fait), `examples/advanced/mini_project/configs/Database.oc` (chemin relatif inconditionnel, redevenu simple), [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) (contexte complet, vérification finale), [runtime-android-aarch64-pic-string-concat](runtime-android-aarch64-pic-string-concat.md) (le bug suivant, distinct, découvert après celui-ci).
