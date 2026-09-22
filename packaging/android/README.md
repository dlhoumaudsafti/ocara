# Ocara UIHybrid — squelette Android (fondations)

Squelette Android minimal démontrant le patron "hybride WebView" décrit dans
[docs/roadmap.d/packaging-android-webview-hybrid.md](../../docs/roadmap.d/packaging-android-webview-hybrid.md) :
une `Activity` Android affiche une `WebView` pointée sur un serveur HTTP
Ocara embarqué, démarré via un pont JNI (`runtime_android_jni`, à la racine
du dépôt).

**État** : fondations posées et vérifiées statiquement (`.so` chargeable par
Bionic, symbole JNI exporté, APK qui compile). **Jamais testé sur un appareil
ou un émulateur réel** — voir [docs/android.md](../../docs/android.md) pour
l'état complet du support Android.

Ce squelette n'est PAS encore le builtin `ocara.UIHybrid` envisagé dans
[docs/roadmap.d/langage-builtin-ui-hybride.md](../../docs/roadmap.d/langage-builtin-ui-hybride.md)
— tout ici est câblé à la main (nom de classe JNI figé, port du serveur figé
dans `MainActivity.kt`), pas encore une abstraction du compilateur Ocara.

## Structure

- `app/src/main/java/com/ocara/bridge/OcaraBridge.kt` — déclaration Kotlin du
  pont JNI (`external fun nativeStartServer()`), charge `libmain.so`.
- `app/src/main/java/com/ocara/demo/MainActivity.kt` — démarre le pont au
  lancement, sonde `http://127.0.0.1:8081/` jusqu'à ce que le serveur réponde,
  puis affiche une `WebView` dessus.
- `app/src/main/res/xml/network_security_config.xml` — autorise le HTTP en
  clair, mais UNIQUEMENT vers `127.0.0.1` (Android ≥ 9 le bloque par défaut).
- `app/src/main/jniLibs/arm64-v8a/` — où placer `libmain.so` (voir ci-dessous).
  **Ignoré par git** (`.gitignore`) : un binaire compilé, jamais une source.

## Exemple réel : `examples/advanced/mini_project`

`examples/advanced/mini_project/main_android.oc` est une variante Android de
`main.oc` — même application (mêmes `configs/`/`controllers/`/`models/`/
`services/`/`templates/`, même base SQLite), mais sans `ocara.Tauri` : `main`
appelle directement `server.start()` (déjà hors du thread UI Android, voir
le pont JNI). Vérifié bout en bout : compile et répond `HTTP 200` avec le
vrai HTML rendu (liste de voitures) une fois lancé sur l'hôte ; cross-compilé
et lié en `.so` pour Android avec la même rigueur que le reste du chantier
(`DT_TEXTREL` absent, symboles JNI/`main` exportés) ; APK construit avec ce
`.so` (~67 Mo, contre ~40 Mo pour le squelette avec un serveur vide — c'est
la vraie application). **Non testé sur un appareil/émulateur réel**, comme le
reste de ce chantier.

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
#    main_android.oc, un serveur ocara.HTTPServer réel sur le port 8081) en .so
#    — lancer depuis le RÉPERTOIRE DU PROGRAMME, pas la racine du dépôt :
#    HTML::renderFile résout les templates relativement au répertoire courant
#    à la compilation, pas au fichier source.
cd examples/advanced/mini_project   # ou le répertoire de votre propre programme
/chemin/vers/ocara main_android.oc \
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

## Limitations connues de ce squelette

- Port du serveur (`8081`) et nom du `.so` (`libmain.so`) codés en dur dans
  `MainActivity.kt`/`OcaraBridge.kt` — aucun besoin connu de les rendre
  configurables pour cette première version.
- **Fichiers statiques non servis** (`self.rootPath("./public/")` dans
  `configs/Server.oc`, ex. `style.css`) : `ocara.HTTPServer` les lit avec les
  API fichier standard (`std::fs`), qui ne voient PAS les assets packagés par
  Gradle (`app/src/main/assets/`, accessibles uniquement via l'API NDK
  `AAssetManager`, un mécanisme distinct d'un vrai chemin de système de
  fichiers). Les pages HTML dynamiques (rendues depuis les templates, EUX
  embarqués en dur dans le binaire à la compilation via le désucrage
  `HTML::renderFile`, donc déjà présents dans le `.so`) fonctionnent
  normalement ; seule la feuille de style ne charge pas. Piste : voir
  [packaging-android-native-features](../../docs/roadmap.d/packaging-android-native-features.md)
  (domaine "fichiers").
- Cycle de vie Android (pause/reprise, arrêt propre du serveur natif) non géré.
- Jamais exécuté sur un appareil/émulateur réel — voir la vérification
  statique complète dans
  [docs/roadmap.d/packaging-android-webview-hybrid.md](../../docs/roadmap.d/packaging-android-webview-hybrid.md).
