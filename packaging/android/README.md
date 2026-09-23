# Ocara UIHybrid — squelette Android (fondations)

Squelette Android minimal démontrant le patron "hybride WebView" décrit dans
[docs/roadmap.d/packaging-android-webview-hybrid.md](../../docs/roadmap.d/packaging-android-webview-hybrid.md) :
une `Activity` Android affiche une `WebView` pointée sur un serveur HTTP
Ocara embarqué, démarré via un pont JNI (`runtime_android_jni`, à la racine
du dépôt).

**État** : fonctionne bout en bout, vérifié en conditions réelles sur un
émulateur x86_64 (KVM) ET sur un appareil physique réel (arm64-v8a) — chargement
JNI, serveur HTTP, SQLite, fichiers statiques, navigation WebView, rendu HTML
tous corrigés et vérifiés (`curl` sur l'appareil réel répond `HTTP 200, 2306
octets`, identique à x86_64 — le dernier bug corrigé était des pointeurs de
tas Android tagués, pas du codegen comme d'abord supposé) — voir
[docs/android.md](../../docs/android.md) et
[docs/roadmap.d/runtime-android-aarch64-pic-string-concat.md](../../docs/roadmap.d/runtime-android-aarch64-pic-string-concat.md)
pour l'état complet.

Ce squelette n'est PAS encore le builtin `ocara.UIHybrid` envisagé dans
[docs/roadmap.d/langage-builtin-ui-hybride.md](../../docs/roadmap.d/langage-builtin-ui-hybride.md)
— tout ici est câblé à la main (nom de classe JNI figé, port du serveur figé
dans `MainActivity.kt`), pas encore une abstraction du compilateur Ocara.

## Structure

- `app/src/main/java/com/ocara/bridge/OcaraBridge.kt` — déclaration Kotlin du
  pont JNI (`external fun nativeStartServer(dataDir: String): Boolean`), charge
  `libmain.so`.
- `app/src/main/java/com/ocara/demo/MainActivity.kt` — copie `assets/` vers le
  répertoire de stockage interne de l'app (voir "Fichiers statiques"
  ci-dessous), démarre le pont au lancement, sonde `http://127.0.0.1:8081/`
  jusqu'à ce que le serveur réponde, puis affiche une `WebView` dessus
  (`WebViewClient()` assigné pour que les clics sur des liens et les
  soumissions de formulaire restent dans la WebView au lieu d'ouvrir le
  navigateur externe — le comportement par défaut d'Android sans ça).
- `app/src/main/res/xml/network_security_config.xml` — autorise le HTTP en
  clair, mais UNIQUEMENT vers `127.0.0.1` (Android ≥ 9 le bloque par défaut).
- `app/src/main/jniLibs/arm64-v8a/` (et `x86_64/` pour tester sur l'émulateur
  accéléré KVM) — où placer `libmain.so` (voir ci-dessous). **Ignoré par git**
  (`.gitignore`) : un binaire compilé, jamais une source.
- `app/src/main/assets/` — fichiers statiques du programme Ocara (CSS,
  images…) à copier ICI À LA MAIN avant de construire l'APK (voir "Fichiers
  statiques" ci-dessous). Contrairement aux `.so`, PAS ignoré par git : ce
  sont des sources (une copie d'un dossier `public/` du projet Ocara), pas un
  artefact de build.

## Exemple réel : `examples/advanced/mini_project`

`examples/advanced/mini_project/main.oc` est multi-plateforme : une branche
`if System::OS equal "android" { server.start(); result 0 }` en tête de
`main` court-circuite le sondage HTTP + `ocara.Tauri` de la version desktop
(le serveur tourne déjà hors du thread UI Android grâce au pont JNI, pas
besoin d'un sous-thread + sondage pour ne pas geler une fenêtre Tauri ouverte
trop tôt) — une seule source portable, pas deux fichiers `.oc` séparés à
maintenir en parallèle.

Vérifié bout en bout, y compris sur un appareil physique réel (pas seulement
l'émulateur) : compile et répond `HTTP 200` avec le vrai HTML rendu (liste de
voitures, SQLite, `public/style.css`) une fois lancé sur l'hôte ; cross-compilé
et lié en `.so` pour Android avec la même rigueur que le reste du chantier
(`DT_TEXTREL` absent, symboles JNI/`main` exportés) ; APK construit avec ce
`.so` (~67 Mo, contre ~40 Mo pour le squelette avec un serveur vide — c'est
la vraie application) ; installé et lancé sur un vrai téléphone Android
(arm64-v8a) : processus vivant, base SQLite créée, fichiers statiques servis,
navigation WebView qui reste dans l'app, page HTML complète et correcte
(`HTTP 200, 2306 octets` — un bug de rendu antérieur, pointeurs de tas Android
tagués, est corrigé et vérifié ici même, voir
[docs/roadmap.d/runtime-android-aarch64-pic-string-concat.md](../../docs/roadmap.d/runtime-android-aarch64-pic-string-concat.md)).

`main.oc` importe `ocara.Tauri` pour sa branche desktop — le compilateur
compile désormais chaque builtin `Tauri_*` en talon local no-op sur Android
(un simple avertissement est émis) plutôt que de rejeter catégoriquement la
compilation, ce qui permet ce point d'entrée unique desktop/Android sans deux
fichiers séparés. Voir
[docs/roadmap.d/packaging-android-webview-hybrid.md](../../docs/roadmap.d/packaging-android-webview-hybrid.md).

## Construire un `.so` et le déposer ici

Depuis la racine du dépôt, avec le NDK déjà installé
(voir [docs/android.md](../../docs/android.md)) :

```bash
NDK=/chemin/vers/android-ndk-rXX

# 1. Runtime Ocara + pont JNI pour aarch64-linux-android (une fois, ou après
#    modification de runtime/ ou runtime_android_jni/)
ANDROID_NDK_HOME="$NDK" make build-runtime-android
ANDROID_NDK_HOME="$NDK" make build-jni-bridge-android

# 2. Compiler + lier VOTRE programme Ocara (ex: examples/advanced/mini_project/
#    main.oc, un serveur ocara.HTTPServer réel sur le port 8081) en .so — lancer
#    depuis le RÉPERTOIRE DU PROGRAMME, pas la racine du dépôt : HTML::renderFile
#    résout les templates relativement au répertoire courant à la compilation,
#    pas au fichier source.
cd examples/advanced/mini_project   # ou le répertoire de votre propre programme
/chemin/vers/ocara main.oc \
  --target aarch64-linux-android \
  --android-runtime /chemin/vers/target/aarch64-linux-android/release/libocara_runtime.a \
  --android-jni-bridge /chemin/vers/target/aarch64-linux-android/release/libocara_runtime_android_jni.a \
  --android-ndk "$NDK" \
  -o /chemin/vers/packaging/android/app/src/main/jniLibs/arm64-v8a/libmain.so

# 3. Construire l'APK (depuis packaging/android/)
cd packaging/android
./gradlew assembleDebug
# → app/build/outputs/apk/debug/app-debug.apk
```

Équivalent condensé pour `examples/advanced/mini_project` : `make android` (les deux ABI, un seul APK) depuis ce répertoire — voir son `Makefile`.

**Production** (APK signé, minifié) : `assembleDebug`/`make android` ci-dessus produisent un APK **debug**, jamais destiné à être distribué. Voir [docs/android.md](../../docs/android.md) §5 pour la procédure complète (génération du keystore, variables d'environnement requises) et `make android-production` (`examples/advanced/mini_project/Makefile`).

## Fichiers statiques : étape manuelle obligatoire

Tout fichier statique servi par `self.rootPath("./public/")` (`configs/Server.oc`,
ex. `style.css`) doit être copié À LA MAIN sous `app/src/main/assets/` **avant**
`./gradlew assembleDebug` — Gradle ne sait pas automatiquement qu'un projet
Ocara a un dossier `public/` à embarquer, et ce squelette ne l'automatise pas
non plus (aucun besoin connu de le faire pour un exemple de démonstration).

Pourquoi une copie et pas un accès direct : un asset Android packagé par
Gradle n'est PAS un chemin de système de fichiers ordinaire (accessible
uniquement via l'API NDK `AAssetManager`, jamais via `std::fs`/`ocara.HTTPServer`).
`MainActivity.onCreate()` copie donc `app/src/main/assets/` vers le répertoire
de stockage interne de l'app (`filesDir`, le même répertoire que celui utilisé
par `chdir()` dans le pont JNI pour `"./app.db"`) à chaque lancement, AVANT de
démarrer le serveur Ocara — `self.rootPath("./public/")` retrouve alors un
vrai chemin de système de fichiers.

Exemple pour `mini_project` : `cp examples/advanced/mini_project/public/style.css packaging/android/app/src/main/assets/public/style.css`.

## Limitations connues de ce squelette

- Port du serveur (`8081`) et nom du `.so` (`libmain.so`) codés en dur dans
  `MainActivity.kt`/`OcaraBridge.kt` — aucun besoin connu de les rendre
  configurables pour cette première version.
- Cycle de vie Android (pause/reprise, arrêt propre du serveur natif) non géré.
