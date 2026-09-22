# Applications Android hybrides (WebView + serveur HTTP Ocara embarqué)

## Fondations — Fait, vérifié (pont JNI + squelette Gradle, APK réel produit)

Voir "Ce qui manque" ci-dessous, points 1 et 2 : **faits et vérifiés**. Le reste (points 3-5 : cleartext HTTP, permission, cycle de vie) est traité de façon minimale mais fonctionnelle pour ce squelette — voir le détail plus bas.

**Bilan global à ce stade** : fonctionne bout en bout sur un émulateur x86_64 accéléré (voir "Test réel sur émulateur"). **Ne fonctionne PAS encore sur un appareil physique réel** (voir "Test réel sur appareil physique") — un vrai bug reproductible, caractérisé, pas juste "non testé" ; voir le ticket dédié [runtime-android-exit-unsafe](runtime-android-exit-unsafe.md).

## Idée

Rendre un programme Ocara du style [examples/advanced/mini_project](../../examples/advanced/mini_project) (serveur `ocara.HTTPServer` + contrôleurs/modèles + templates HTML rendus côté serveur) **fonctionnel sur Android**, en l'affichant dans une `WebView` Android système pointée sur le serveur HTTP embarqué (`http://127.0.0.1:<port>/`) — même principe que Cordova/Capacitor, ou que ce que Tauri fait sur desktop (une fenêtre native + un moteur web, mais le "backend" est du code Ocara natif, pas du JS).

C'est un point de départ délibérément plus étroit et plus sûr que le support SDL/GUI native (voir [packaging-android](packaging-android.md) sous-chantier 4 et le ticket suivant sur la GUI native) : **aucune dépendance à SDL3, GTK ou Tauri** — un serveur HTTP est juste des sockets, déjà vérifié comme cross-compilable pour Bionic ([packaging-android](packaging-android.md) sous-chantier 3, `ocara_runtime` de base). Le morceau réellement nouveau ici est le pont JNI + le squelette Android (Activity/WebView/Gradle), pas le runtime Ocara lui-même.

## Ce qui est déjà en place (vérifié par [packaging-android](packaging-android.md))

- Cross-compilation Cranelift vers `aarch64-linux-android` (sous-chantier 1).
- `ocara_runtime` (donc `ocara.HTTPServer`, sockets, SQLite, HTML) compile pour Bionic (sous-chantier 3).
- Liaison en `.so` partagé, PIC, sans `DT_TEXTREL`, chargeable par le dynamic linker Bionic (sous-chantier 2).
- Un `.so` Ocara exporte déjà `main`/`__fn_wrap_main` comme symboles ordinaires (confirmé par `objdump -t` lors de la vérification du sous-chantier 2) — un point d'entrée qu'un pont JNI peut appeler directement, sans modification du compilateur.

## Ce qui manque, non vérifié (état avant travail) / Ce qui a été fait

1. **Pont JNI — Fait.** Nouveau crate `runtime_android_jni` (staticlib, dépendance unique `jni-sys` — bindings FFI bruts, pas le wrapper haut niveau `jni`, inutile ici puisqu'aucun objet Java n'est manipulé). Expose `Java_com_ocara_bridge_OcaraBridge_nativeStartServer` (convention de nommage JNI, classe Java figée plutôt que configurable — voir la doc du crate) : lance `main()` du programme Ocara lié dans le même `.so` sur un thread séparé, fire-and-forget. **Piège réel rencontré** : rien ne référence ce symbole ailleurs dans le `.so` (seule la JVM l'appelle, dynamiquement, à l'exécution) — l'extraction paresseuse habituelle d'une archive `.a` le laissait silencieusement de côté (confirmé par reproduction : absent de `nm -D` sur le premier essai). Corrigé avec `-Wl,-u,<symbole>` dans `link_android` (nouveau paramètre `jni_bridge_lib` + flag CLI `--android-jni-bridge`), qui force son extraction même sans référence entrante.
2. **Squelette Android (Activity + Gradle) — Fait.** `packaging/android/` : projet Gradle réel (scaffoldé via l'outil `android` du SDK, template `empty-activity`, Compose), simplifié à l'essentiel — `MainActivity.kt` démarre le pont JNI puis sonde `http://127.0.0.1:8081/` avant d'afficher une `WebView` (`AndroidView` Compose) dessus, `OcaraBridge.kt` déclare le pont côté Kotlin.
3. **Cleartext HTTP local — Traité, scope minimal.** `network_security_config.xml` autorise le HTTP en clair, mais UNIQUEMENT vers `127.0.0.1` (pas un `usesCleartextTraffic` global) — fonctionne sur le `minSdk` retenu (24 ; Network Security Config existe depuis l'API 24). Pas d'alternative HTTPS explorée, jugée inutile pour une boucle locale scoped ainsi.
4. **Permission réseau — Fait.** `<uses-permission android:name="android.permission.INTERNET" />` dans le manifeste.
5. **Cycle de vie Android — PAS traité**, comme anticipé : le pont reste fire-and-forget (aucun arrêt propre du serveur natif à la fermeture de l'Activity, aucune notification "serveur prêt" autre que le sondage HTTP côté Kotlin).

### Vérification

Sans appareil ni émulateur Android (aucun disponible dans cet environnement), vérifié statiquement à chaque étape :

- `libocara_runtime_android_jni.a` cross-compilé pour `aarch64-linux-android` : `nm` sur l'objet interne confirme `Java_com_ocara_bridge_OcaraBridge_nativeStartServer` exporté (`T`, texte global).
- Un programme Ocara de test (`ocara.HTTPServer` minimal) lié avec `--android-jni-bridge` en plus de `--android-runtime` : `.so` final vérifié avec la même rigueur que [packaging-android](packaging-android.md) (ELF `DYN` AArch64, **aucun `DT_TEXTREL`**, `nm -D` confirme cette fois `Java_com_ocara_bridge_OcaraBridge_nativeStartServer` ET `main` tous deux présents et exportés dans la table de symboles dynamiques).
- **APK réel construit** : `./gradlew assembleDebug` → `BUILD SUCCESSFUL`, `app-debug.apk` valide (`file` : "Android package (APK)"). Contenu vérifié avec `apkanalyzer` : `lib/arm64-v8a/libmain.so` présent (taille exactement identique au `.so` vérifié séparément), permission `INTERNET` présente dans le manifeste compilé, classes `com.ocara.bridge.OcaraBridge` (avec `nativeStartServer()`) et `com.ocara.demo.MainActivity` bien compilées dans le dex.
- **Non vérifiable ici** : installation et exécution réelles sur un appareil/émulateur (chargement JNI effectif, rendu WebView, sondage réseau réel) — nécessite un device ou un émulateur configuré, hors de portée de cet environnement.

### Découverte d'outillage : le SDK Android fournit désormais un CLI agent-friendly

Le SDK téléchargé (`commandlinetools-linux`) inclut un nouvel outil `android` (au-delà du `sdkmanager` classique, marqué déprécié) avec des sous-commandes `create` (scaffold un projet depuis un template), `sdk` (gestion des paquets), `run`/`install` (déploiement sur device/émulateur), `emulator` (gestion des AVD). `android create empty-activity` a scaffoldé tout le projet Gradle (wrapper, `build.gradle.kts`, AGP 9.x, Compose) en une commande, y compris l'installation automatique de `platforms;android-36` manquant — évite d'écrire à la main des fichiers Gradle potentiellement erronés/datés.

## Adaptation réelle de `examples/advanced/mini_project` — Fait, vérifié

**Terminé.** `examples/advanced/mini_project/main_android.oc` : même application que `main.oc` (mêmes `configs/`/`controllers/`/`models/`/`services/`/`templates/`, même base SQLite) mais sans `ocara.Tauri` — `main` appelle directement `server.start()` (déjà hors du thread UI Android grâce au pont JNI, pas besoin du sous-thread + sondage HTTP que fait la version desktop pour ne pas geler une fenêtre Tauri ouverte trop tôt).

### Vérification

- **Sur l'hôte d'abord** (avant tout effort de cross-compilation, pour isoler les bugs applicatifs des bugs Android) : compilé et exécuté directement, `curl http://localhost:8081/` répond `HTTP 200` avec le vrai HTML rendu (liste de voitures, SQLite, templates) — confirme que l'adaptation du point d'entrée n'a rien cassé.
- **Cross-compilé et lié pour Android** avec `--android-jni-bridge`, même rigueur que le reste du chantier : `.so` ELF `DYN` AArch64, **aucun `DT_TEXTREL`**, `main` et `Java_com_ocara_bridge_OcaraBridge_nativeStartServer` tous deux exportés.
- **APK réel construit** (`./gradlew assembleDebug`, `BUILD SUCCESSFUL`) avec ce `.so` — taille APK ~67 Mo (contre ~40 Mo pour le squelette de démonstration avec un serveur vide), cohérent avec une vraie application (contrôleurs, modèles, templates HTML, SQLite) plutôt qu'un stub.

### Découverte durant l'adaptation : fichiers statiques non servis sur Android

`self.rootPath("./public/")` (`configs/Server.oc`, sert `style.css`) ne fonctionnera PAS tel quel sur un appareil réel : `ocara.HTTPServer` lit ces fichiers avec les API fichier standard (`std::fs`), qui ne voient pas les assets packagés par Gradle (`app/src/main/assets/`, accessibles uniquement via l'API NDK `AAssetManager`, un mécanisme distinct d'un chemin de système de fichiers classique). Les pages HTML dynamiques restent intactes : les templates sont désucrés à la compilation par `HTML::renderFile` en chaînes littérales embarquées DANS le `.so` — seule la feuille de style externe est concernée. Piste de résolution : [packaging-android-native-features](packaging-android-native-features.md) (domaine "fichiers").

## Test réel sur émulateur — Fait, vérifié

**Terminé.** Le SDK Android fournit un vrai gestionnaire d'émulateurs (`android emulator create/start/stop/list`), et **KVM est disponible sur cet environnement** (`/dev/kvm` accessible) — un émulateur **x86_64** (accélération matérielle complète) a été créé, démarré (headless, ~80s de démarrage, confirmant une vraie accélération), et utilisé pour un test réel de bout en bout, pas seulement de l'inspection statique.

Piège d'ABI important : l'APK ne contenait qu'un `.so` **arm64-v8a**. Un hôte x86_64 émule de l'ARM64 en traduction logicielle pure (pas d'accélération KVM pour une ISA différente de celle de l'hôte), potentiellement très lent — la bonne combinaison pour un test rapide et réactif est un système x86_64 (accéléré) + un `.so` **x86_64**. `ocara` supporte déjà `--target x86_64-linux-android` nativement (le backend x86 de Cranelift est déjà embarqué via `host-arch`, cet hôte étant lui-même x86_64 — aucune feature Cargo supplémentaire à activer, contrairement à `arm64` pour le sous-chantier 1). Les cibles Make (`build-runtime-android`, `build-jni-bridge-android`, `build-runtime-sdl-android`) ont été rendues paramétrables (`ANDROID_TARGET`, défaut `aarch64-linux-android`) pour cross-compiler aussi vers `x86_64-linux-android` sans dupliquer chaque cible.

Nouvelle cible **`make android-simulator <apk>`** (voir `Makefile`) : vérifie qu'un AVD existe (le crée sinon), qu'un émulateur tourne réellement (`adb devices`, pas la colonne Status de `emulator list` qui ne reflète pas l'état réel), désinstalle une éventuelle version déjà installée (purge données/cache) pour repartir d'un état propre, installe l'APK, puis le lance (`monkey -c android.intent.category.LAUNCHER`, sans avoir besoin de connaître le nom exact de l'Activity).

### Deux bugs réels trouvés et corrigés grâce à ce test (invisibles par inspection statique)

Le premier lancement de l'APK **plantait** (`SIGABRT`), un résultat que ni la vérification statique ni les tests hôte n'auraient jamais pu révéler :

1. **`System::OS` ne reconnaissait pas Android.** `runtime/src/lib.rs`, `__system_os()` : `cfg!(target_os = "linux")` est **`false`** pour une cible `aarch64-linux-android`/`x86_64-linux-android` — vérifié (`rustc --print cfg --target x86_64-linux-android` ne donne QUE `target_os="android"`, jamais `"linux"` en plus), alors que le triple contient pourtant le mot "linux". Sans branche dédiée, `System::OS` retournait `"unknown"` sur Android. Corrigé (ajout de `cfg!(target_os = "android")` avant les autres branches) — `docs/builtins/System.md`/`src/builtins/system.rs` mis à jour (`"android"` ajouté aux valeurs possibles).
2. **Chemin SQLite invalide au démarrage.** `configs/Database.oc` retournait `"./app.db"` (chemin relatif) inconditionnellement — sur Android, `SQLite::open()` échouait (répertoire de travail non accessible en écriture), déclenchant `throw_sqlite_exception` → `__ocara_fail` → `std::process::exit`, qui a lui-même révélé un **second problème distinct**, plus profond : la séquence de sortie détruit un mutex encore possédé/contesté (`mutex.cc:432] destroying mutex with owner or contenders`) — glibc tolère silencieusement ce comportement indéfini (UB), Bionic le détecte explicitement et force un `SIGABRT`. Corrigé pour ce ticket en rendant `Database::path()` conditionnel (`if System::OS equal "android"`) vers un chemin absolu dans le bac à sable de l'app (`/data/user/0/com.ocara.demo/app.db`) — **piège concret rencontré en le corrigeant** : un premier essai avec `.../files/app.db` échouait ENCORE, car `files/` n'est créé par Android que lors du premier appel à `Context.getFilesDir()` côté Java (jamais fait ici) — confirmé par reproduction (`adb shell run-as com.ocara.demo ls -la /data/user/0/com.ocara.demo/` : seuls `cache/`/`code_cache/` existent). Le répertoire racine du bac à sable, lui, existe toujours dès l'installation.

Le second problème (destruction de mutex non sûre pendant la sortie sur exception non gérée) n'a PAS été creusé plus loin ici — corrigé indirectement en évitant de déclencher `throw_sqlite_exception` du tout, pas en réparant la séquence de sortie elle-même. Un vrai bug de fiabilité runtime, à part, mériterait sa propre investigation un jour (pourquoi `std::process::exit` interagit avec un mutex encore possédé).

### Vérification finale

Sur l'émulateur x86_64 réel, avec le vrai `main_android.oc`/`Database.oc` (fichiers du dépôt, pas une copie de test) : processus vivant (pas de crash), `adb forward` + `curl http://127.0.0.1:<port-forwardé>/` répond `HTTP 200` avec le vrai titre rendu (`<title>Voitures — Garage Ocara</title>`), fichier SQLite créé exactement à l'emplacement attendu (`adb shell run-as ... ls -la /data/user/0/com.ocara.demo/app.db`).

## Test réel sur appareil physique — Tenté, échec reproductible et bien caractérisé

**Pas résolu** — contrairement à l'émulateur, l'app **plante** sur un vrai téléphone (Realme RMX3834, Android 15/API 35, arm64-v8a, branché en USB et détecté automatiquement à côté de l'émulateur). Testé via la même cible `make android-simulator`, étendue avec un nouveau paramètre `ANDROID_SERIAL=<serial>` pour cibler un appareil précis au lieu de gérer un AVD.

Chaîne d'événements observée (deux tentatives consécutives, données réelles via `adb pull /data/tombstones/...`, pas une supposition) :
1. Premier lancement : `Fatal signal 11 (SIGSEGV)` dans un thread **"Jit thread pool"**, backtrace entièrement à l'intérieur d'ART (`art::jit::JitCodeCache::Commit`, `/apex/com.android.art/lib64/libart.so`) — **sans rapport avec le code Ocara** (le `.so` venait tout juste de finir de charger, rien de notre code n'avait encore eu la main). Vraisemblablement un problème ART/ROM (Realme/ColorOS) préexistant, pas causé par ce projet.
2. Android relance automatiquement l'app après ce crash (comportement standard). Cette fois, `Database::connect()` → `SQLite::open()` échoue et l'app replante — de façon **identique après un `pm clear` complet** (état totalement propre, élimine l'hypothèse d'un fichier corrompu par le crash précédent).

Ce second crash a révélé un problème plus profond et plus général que le seul chemin SQLite : voir le nouveau ticket dédié **[runtime-android-exit-unsafe](runtime-android-exit-unsafe.md)** — `__ocara_fail` (le gestionnaire d'exception non rattrapée) appelle `std::process::exit()`, ce qui est sûr sur desktop mais fait s'effondrer le processus sur Android (`__cxa_finalize` détruit des objets globaux d'ART/`libhwui` jamais conçus pour l'être en cours de vie d'une app — deux variantes de crash observées, `art::Mutex::~Mutex()` et `android::uirenderer::CommonPool::~CommonPool()`, selon quel composant système est détruit en premier).

La cause racine de l'échec `SQLite::open()` lui-même **n'a pas pu être déterminée** : le répertoire cible a un propriétaire/contexte SELinux corrects (vérifié), la taille de page mémoire est standard (4096, pas 16 Ko), mais **aucun message d'erreur du runtime Ocara n'est visible dans `logcat`** (le `stderr` d'un `.so` chargé par JNI n'est pas capturé par le système de log Android) — sans cette visibilité, continuer à deviner des chemins serait la même méthode par tâtonnement déjà insuffisante ici. Détails complets : [runtime-android-exit-unsafe](runtime-android-exit-unsafe.md).

## Ce qui reste (hors "fondations", adaptation de mini_project et test émulateur)

- **Fonctionne sur émulateur (x86_64), pas encore sur appareil physique réel** (arm64-v8a) — voir ci-dessus, non résolu.
- Cycle de vie Android (voir point 5 plus haut).
- Fichiers statiques Android (voir découverte plus haut).
- `__ocara_fail` dangereux sur Android (voir [runtime-android-exit-unsafe](runtime-android-exit-unsafe.md), nouveau ticket dédié) — probablement le VRAI blocage pour tout test futur sur appareil réel, pas seulement pour SQLite.
- Aucune visibilité sur les logs/erreurs du runtime Ocara depuis un `.so` Android (voir le même ticket) — prérequis pour diagnostiquer correctement ce type de problème à l'avenir plutôt que par tâtonnement.
- Généralisation en builtin `ocara.UIHybrid`, voir [langage-builtin-ui-hybride](langage-builtin-ui-hybride.md) — tout ici reste câblé à la main (port et nom de `.so`/package codés en dur, DEUX points d'entrée `.oc` séparés pour desktop/Android plutôt qu'un seul source portable), pas une abstraction du compilateur.

## Priorité / Complexité

**Priorité Très Basse** — étape "pour commencer" explicitement voulue par David avant la GUI native. Fondations posées et vérifiées sur émulateur x86_64 (accéléré KVM) ; **pas encore fonctionnel sur appareil physique réel** (arm64-v8a), un échec reproductible et bien caractérisé plutôt qu'un simple "pas testé". **Complexité** : le pont JNI, le squelette Gradle et le test émulateur, une fois les bons outils trouvés (le CLI `android`), se sont avérés une **addition contenue** ; le blocage sur appareil réel, lui, touche un mécanisme plus profond (gestion des exceptions non rattrapées, voir [runtime-android-exit-unsafe](runtime-android-exit-unsafe.md)) dont la complexité réelle n'est pas encore évaluée.

## Fichiers clés

`runtime_android_jni/` (pont JNI, **fait**), `src/codegen/link.rs` (`link_android`, paramètre `jni_bridge_lib` + `-Wl,-u`, **fait**), `src/core/cli.rs` (flag `--android-jni-bridge`, **fait**), `Makefile` (cibles `build-jni-bridge-android`/`build-runtime-android`/`build-runtime-sdl-android` paramétrées par `ANDROID_TARGET`, cible `android-simulator` avec option `ANDROID_SERIAL` pour cibler un appareil précis, **fait**), `packaging/android/` (squelette Gradle complet, **fait** — voir son propre `README.md`), `runtime/src/lib.rs` (`__system_os`, ajout de la branche `"android"`, **fait** ; `__ocara_fail`, dangereux sur Android, **pas fait**, voir [runtime-android-exit-unsafe](runtime-android-exit-unsafe.md)), `examples/advanced/mini_project/configs/Database.oc` (chemin SQLite conditionnel par OS, **fait pour l'émulateur, insuffisant sur appareil réel**), [packaging-android](packaging-android.md) (prérequis, les 4 sous-chantiers sont faits), `examples/advanced/mini_project/main_android.oc` (variante Android de l'exemple de référence, **vérifié en exécution réelle sur émulateur, plante sur appareil physique**), [runtime-android-exit-unsafe](runtime-android-exit-unsafe.md) (nouveau ticket dédié, le vrai blocage pour l'appareil réel), [langage-builtin-ui-hybride](langage-builtin-ui-hybride.md) (généralisation en builtin Ocara, pas commencée).
