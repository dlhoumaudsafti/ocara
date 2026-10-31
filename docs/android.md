# Ocara — Compilation croisée vers Android

> Guide pratique pour produire une bibliothèque partagée (`.so`) Android à partir d'un programme `.oc`, avec le compilateur `ocara` compilé normalement pour votre machine (**aucun besoin du NDK pour construire `ocara` lui-même**).

---

## État actuel

| Sous-chantier | État |
|----------------|------|
| Cross-compilation Cranelift (choix de la cible) | ✅ Fait, vérifié |
| Portage du runtime (`ocara_runtime`) vers Bionic | ✅ Fait, vérifié (ABI `aarch64` uniquement) |
| Liaison finale en `.so` (clang du NDK) | ✅ Fait, vérifié |
| Backend Android pour `ocara.SDL` | ✅ Fait, vérifié (formats audio "tracker"/MOD non disponibles) |
| `ocara.Tauri` sur Android | ❌ Hors périmètre définitif (GTK ne tourne pas sur Android) |
| Packaging APK / test sur émulateur (x86_64, KVM) | ✅ Fait, vérifié — voir §4 |
| Builtins natifs (`ocara.Android*` : permissions, notifications, capteurs, fichiers, caméra, micro, audio) | ✅ Fait, vérifié sur appareil réel — voir §6 |
| Test sur appareil physique réel | ✅ Fait, vérifié — crash/fichiers statiques/WebView/rendu HTML tous corrigés (`HTTP 200, 2306 octets`, identique à x86_64) — voir §4 et [runtime-android-aarch64-pic-string-concat](roadmap.d/runtime-android-aarch64-pic-string-concat.md) |

Tout ce qui est vérifiable **par inspection statique du binaire produit** (architecture ELF correcte, bibliothèque réellement partagée, absence de `DT_TEXTREL`, toutes les dépendances dynamiques déclarées et effectivement fournies par les bibliothèques système du NDK) l'a été — ET, depuis §4, le comportement réel a aussi été vérifié **sur un vrai émulateur** (chargement JNI, `System.loadLibrary`, exécution du programme Ocara, réponse HTTP réelle). Ce SDK a un émulateur x86_64 avec accélération matérielle KVM (voir §4) — pas juste arm64-v8a en émulation logicielle lente. **Testé aussi sur un appareil physique réel (arm64-v8a)** : le crash au démarrage, l'accès aux fichiers statiques, la redirection WebView externe et le rendu HTML corrompu (pointeurs de tas Android tagués, pas un bug de codegen comme d'abord supposé — voir §4 et [roadmap.d/runtime-android-aarch64-pic-string-concat.md](roadmap.d/runtime-android-aarch64-pic-string-concat.md)) sont tous corrigés, pas juste "jamais essayé".

Historique complet des vérifications et des pièges rencontrés : [roadmap.d/packaging-android.md](roadmap.d/packaging-android.md).

---

## Prérequis

| Outil | Rôle | Requis pour |
|-------|------|-------------|
| NDK Android (r23+, tout-LLVM) | clang croisé + sysroot Bionic | §1, §2 — cross-compiler et lier |
| Cible Rust `aarch64-linux-android` (`rustup target add aarch64-linux-android`) | Compiler `ocara_runtime` pour Bionic | §1 |
| SDK Android (cmdline-tools) + JDK 17+ | `android`/`sdkmanager`, Gradle (via le wrapper du projet) | §4 uniquement (construire un APK) |

`ocara` lui-même **n'a pas besoin** du NDK pour être construit (`make build` reste inchangé) — le NDK ne sert qu'à (a) cross-compiler le runtime Ocara une fois, et (b) lier le programme final. Le SDK Android complet n'est nécessaire que pour packager un APK (§4) — produire et vérifier un `.so` (§1-§3) ne le requiert pas.

Seule l'ABI `arm64-v8a` (triple `aarch64-linux-android`) a été vérifiée — de loin la plus pertinente aujourd'hui (quasi tous les appareils Android actuels). `armv7-linux-androideabi`/`x86_64-linux-android`/`i686-linux-android` sont reconnus par le CLI mais **non testés**.

### Installer le NDK

```bash
# URL/empreinte à jour : dérivées du manifeste officiel de Google
# (https://dl.google.com/android/repository/repository2-3.xml, packages
# "ndk;<version>", canal "stable") plutôt que codées en dur indéfiniment —
# la commande ci-dessous correspond à la version installée et vérifiée pour
# ce projet (r30) au moment de la rédaction.
curl -L -o android-ndk.zip https://dl.google.com/android/repository/android-ndk-r30-linux.zip
echo "5107f898313790e449e87eee2183d9a20602dee9  android-ndk.zip" | sha1sum -c -
unzip -q android-ndk.zip -d ~/
rm android-ndk.zip
export ANDROID_NDK_HOME=~/android-ndk-r30
```

### Installer le SDK Android (cmdline-tools) — uniquement pour §4 (APK)

```bash
# Idem : URL/empreinte dérivées du manifeste officiel de Google (package
# "cmdline-tools;<version>", canal "stable") ; version vérifiée pour ce projet
# ci-dessous.
curl -L -o cmdline-tools.zip https://dl.google.com/android/repository/commandlinetools-linux-16111833_latest.zip
echo "e025545c62a8e64c7559119566a569fb1dec5f60  cmdline-tools.zip" | sha1sum -c -
mkdir -p ~/android-sdk/cmdline-tools
unzip -q cmdline-tools.zip -d ~/android-sdk/cmdline-tools_tmp
# Layout exigé par les outils du SDK : cmdline-tools/latest/, pas cmdline-tools/ directement.
mv ~/android-sdk/cmdline-tools_tmp/cmdline-tools ~/android-sdk/cmdline-tools/latest
rmdir ~/android-sdk/cmdline-tools_tmp
rm cmdline-tools.zip
export ANDROID_HOME=~/android-sdk
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"
```

> **Découverte** : ce SDK fournit désormais un outil `android` (le `sdkmanager` classique est marqué déprécié à l'exécution) avec des sous-commandes agent-friendly — `android create empty-activity --namespace ... --min-sdk 24 --output packaging/android` a scaffoldé tout le squelette Gradle de `packaging/android/` (wrapper, `build.gradle.kts`, AGP, Compose) en une seule commande, y compris l'installation automatique de `platforms;android-36` manquant. `android sdk install "platforms;android-36" "build-tools;36.0.0"` pour installer des paquets individuellement. Voir `android --help`.

Gradle lui-même n'a **pas** besoin d'une installation séparée : `packaging/android/gradlew` (généré par `android create`) télécharge et épingle sa propre version au premier lancement.

---

## 1. Compiler le runtime Ocara pour Android (une seule fois)

```bash
export ANDROID_NDK_HOME=/chemin/vers/android-ndk-rXX
make build-runtime-android
```

Produit `target/aarch64-linux-android/release/libocara_runtime.a`. À refaire uniquement si le runtime (`runtime/`) change — pas à chaque compilation d'un programme Ocara.

> **Piège connu** : les NDK ≥ r23 (tout-LLVM) ne fournissent plus les wrappers binutils préfixés par triple (`aarch64-linux-android-ranlib`). Si vous adaptez cette cible Make, gardez `RANLIB_aarch64_linux_android` pointé explicitement vers le `llvm-ranlib` du NDK — sinon la compilation d'OpenSSL (vendored) échoue en toute fin de build avec `aarch64-linux-android-ranlib: not found`.

### Si le programme importe `ocara.SDL`

```bash
make build-runtime-sdl-android
```

Produit `target/aarch64-linux-android/release/libocara_runtime_sdl.a` (SDL3 + image/ttf/mixer). Plus long (compile SDL3 et ses dépendances C depuis les sources).

> **Pièges connus** (voir [roadmap.d/packaging-android.md](roadmap.d/packaging-android.md) sous-chantier 4 pour le détail complet) : le codec audio "tracker"/MOD de SDL_mixer (`libxmp`) est désactivé pour Android (bug d'édition de liens avec `ld.lld` sur sa variante partagée, puis un second bug CMake/rpkg-config sur sa variante statique — WAV/OGG/MP3/FLAC/Opus restent disponibles) ; un compilateur C++ explicite (`CXX_aarch64_linux_android`) est requis en plus du C (au moins un codec, `gme`, est en C++).

---

## 2. Compiler et lier un programme `.oc` en `.so` Android

```bash
ocara main.oc \
  --target aarch64-linux-android \
  --android-runtime target/aarch64-linux-android/release/libocara_runtime.a \
  --android-ndk "$ANDROID_NDK_HOME" \
  -o libmain.so
```

| Option | Rôle |
|--------|------|
| `--target <triple>` | Cible Cranelift (ex: `aarch64-linux-android`) — voir aussi `--no-link` ci-dessous pour n'obtenir qu'un `.o` |
| `--android-runtime <fichier.a>` | Le `libocara_runtime.a` produit à l'étape 1 |
| `--android-runtime-sdl <fichier.a>` | Le `libocara_runtime_sdl.a` produit ci-dessus — requis seulement si le programme importe `ocara.SDL` |
| `--android-ndk <dir>` | Racine du NDK (défaut : `$ANDROID_NDK_HOME`) |

Le `.so` produit dépend dynamiquement de `libc.so`/`libm.so`/`libz.so`/`libdl.so` — toutes fournies par n'importe quel appareil Android. **Avec `ocara.SDL`**, il dépend en plus de `libandroid.so`/`liblog.so`/`libGLESv2.so`/`libOpenSLES.so`/`libc++_shared.so` : ces cinq-là sont des bibliothèques système NDK standards, mais `libc++_shared.so` en particulier **doit être copiée dans `jniLibs/<abi>/` de l'APK final**, à côté du `.so` Ocara — ce n'est pas un fichier système garanti présent sur l'appareil comme les autres.

### Produire uniquement un `.o` (sans lier)

Utile pour inspecter le code généré sans NDK :

```bash
ocara main.oc --target aarch64-linux-android --no-link -o main
# → main.o, un objet relogeable AArch64 (readelf -h : Machine: AArch64)
```

### Programmes non supportés vers Android

| Import | Comportement |
|--------|--------------|
| `ocara.Tauri` | Rejeté à la liaison — aucun équivalent Android (GTK), hors périmètre définitif |
| `ocara.SDL` sans `--android-runtime-sdl` | Rejeté à la liaison — évite un `.so` avec des symboles SDL non résolus |

Ces deux cas sont détectés et refusés explicitement plutôt que de produire un `.so` silencieusement cassé.

---

## 3. Vérifier un `.so` produit (sans appareil Android)

```bash
# Type et architecture
file libmain.so
# → ELF 64-bit LSB shared object, ARM aarch64, ..., dynamically linked

# Absence de DT_TEXTREL (CRITIQUE : Bionic refuse de charger un .so qui en a un)
readelf -d libmain.so | grep -i textrel
# → rien affiché = OK

# Dépendances dynamiques déclarées
readelf -d libmain.so | grep NEEDED

# Symboles non résolus restants (hors libc/libm/libz/libdl et hors symboles "weak")
nm -D --undefined-only libmain.so | grep -v '@LIBC' | awk '$1!="w"'
# → devrait être vide
```

Un symbole `w` (weak, ex. `getrandom`, `copy_file_range`) est normal : c'est un mécanisme de détection de fonctionnalité à l'exécution, résolu à `NULL` si absent, pas une dépendance manquante.

---

## 4. Application hybride (WebView + serveur HTTP embarqué)

Un squelette Android complet (Activity Kotlin + WebView + pont JNI générique) existe dans [packaging/android/](../packaging/android/) — voir son `README.md` pour la marche à suivre complète (compiler le pont JNI avec `make build-jni-bridge-android`, lier un programme Ocara avec `--android-jni-bridge`, construire l'APK avec `./gradlew assembleDebug`). Détails et vérifications complètes : [roadmap.d/packaging-android-webview-hybrid.md](roadmap.d/packaging-android-webview-hybrid.md).

### Nom et icône de l'application

Le nom affiché sous l'icône et l'icône du lanceur viennent du programme
empaqueté, par deux propriétés Gradle (sans elles : ceux de l'app de
démonstration) :

```bash
cd packaging/android && ./gradlew assembleDebug -PocaraAppName="Carfolio" -PocaraIcon=/chemin/logo.png
```

`ocaraIcon` est un PNG carré (idéalement 512 px) : il devient l'icône de
chaque densité (`mipmap-*`) et le premier plan de l'icône adaptative
(réduit à sa zone sûre, sur fond noir). Le SVG n'est pas accepté : le
rendre en PNG d'abord (exemple : `make logo` de
`examples/advanced/mini_project_ui_hexa`, qui passe aussi ces propriétés
à `make android`).

### Tester sur un émulateur

```bash
make android-simulator packaging/android/app/build/outputs/apk/debug/app-debug.apk
```

Vérifie qu'un AVD existe (le crée sinon), qu'un émulateur tourne (le démarre sinon), désinstalle une éventuelle version déjà installée (purge données/cache), installe l'APK et le lance — nécessite `$ANDROID_HOME`. Voir `make help`.

Par défaut : profil `medium_phone`, **x86_64** — pas arm64-v8a — pour avoir l'accélération matérielle KVM (un hôte x86_64 émule arm64 uniquement en traduction logicielle, beaucoup plus lent). Ça veut dire que l'APK testé doit contenir un `.so` **x86_64** (`--target x86_64-linux-android` — supporté nativement par `ocara`, aucune feature Cargo supplémentaire requise, contrairement à `arm64` qui a dû être ajoutée explicitement pour le sous-chantier 1) en plus (ou à la place) de l'arm64-v8a habituel. Les cibles `make build-runtime-android`/`build-jni-bridge-android`/`build-runtime-sdl-android` acceptent toutes `ANDROID_TARGET=x86_64-linux-android` pour ça (défaut : `aarch64-linux-android`).

**Vérifié réellement sur cet émulateur** (`examples/advanced/mini_project`, point d'entrée `main.oc` multi-plateforme, voir [roadmap.d/packaging-android-webview-hybrid.md](roadmap.d/packaging-android-webview-hybrid.md)) : chargement JNI, démarrage du serveur, SQLite, rendu HTML — tout fonctionne. Deux bugs réels trouvés et corrigés au passage :
- `System::OS` ne reconnaissait pas Android (`target_os = "android"` est distinct de `"linux"` pour rustc) et retournait `"unknown"` — corrigé (`runtime/src/lib.rs`), `System::OS` vaut maintenant `"android"`.
- Un chemin SQLite relatif (`"./app.db"`) plantait l'app au démarrage sur cet émulateur avant que le pont JNI fasse `chdir()` vers un répertoire connu (voir juste en dessous) — désormais résolu de façon générale, plus besoin de chemin spécifique à Android.

**Fonctionne aussi sur un appareil physique réel** (arm64-v8a, testé sur un vrai téléphone Android branché en USB — `ANDROID_SERIAL=<serial> make android-simulator <apk>` cible un appareil précis au lieu de gérer un émulateur). Deux problèmes réels ont d'abord fait planter/mal fonctionner l'app sur ce téléphone (jamais reproduits sur l'émulateur), tous deux corrigés et vérifiés :
- **Crash au démarrage** (`__ocara_fail`, le gestionnaire d'exception non rattrapée d'Ocara, appelait `std::process::exit()` — sûr sur desktop, fatal sur Android car ça détruit des objets globaux d'ART/`libhwui`) — corrigé (`libc::_exit(1)` sur Android). Son déclencheur, un échec `SQLite::open()`, a lui aussi été résolu à la racine : le pont JNI fait maintenant `chdir()` vers `Context.getFilesDir()` (obtenu côté Kotlin, la seule API garantissant la création réelle du répertoire et son étiquetage SELinux correct) avant de démarrer le programme Ocara — tout chemin relatif (`"./app.db"`, `"./public/"`) pointe alors vers un endroit qui existe déjà et est accessible en écriture, exactement comme sur desktop. Détails complets : [roadmap.d/runtime-android-exit-unsafe.md](roadmap.d/runtime-android-exit-unsafe.md) (clos).
- **Fichiers statiques (`public/`) et redirection WebView externe** — également corrigés, voir §"Limitations connues" ci-dessous et [roadmap.d/packaging-android-webview-hybrid.md](roadmap.d/packaging-android-webview-hybrid.md).

**Rendu HTML corrompu sur arm64 réel — corrigé.** La page rendue était corrompue (un grand nombre à l'allure d'une adresse mémoire remplaçait une partie du contenu), jamais reproduit sur x86_64 (émulateur ou hôte) ni avec des littéraux. Cause racine réelle : Bionic/Scudo (l'allocateur natif d'Android depuis la version 11) tague l'octet de poids fort des pointeurs de tas, que `ocara_runtime` classait à tort comme un entier brut au lieu d'un pointeur (une première hypothèse de bug de codegen AArch64+PIC était fausse). Corrigé en désactivant ce tagging pour tout le processus (`mallopt` dans le pont JNI) ; vérifié en exécution réelle sur un vrai téléphone (`curl` → `HTTP 200, 2306 octets`, identique à x86_64). Détails complets : [roadmap.d/runtime-android-aarch64-pic-string-concat.md](roadmap.d/runtime-android-aarch64-pic-string-concat.md).

---

## 5. Build de production (APK signé, minifié)

`./gradlew assembleDebug` (§4) produit un APK **debug** : signé avec un keystore auto-généré par Gradle (jamais accepté par le Play Store, ni reconnu comme mise à jour légitime par un appareil ayant déjà l'app installée), non minifié. Un build de **production** utilise `assembleRelease` à la place — signé avec un vrai keystore, réduit par R8 (minification + suppression des ressources inutilisées).

### 5.1 Générer un keystore (une seule fois par projet)

```bash
keytool -genkeypair -v \
  -keystore ~/.android-keystores/ocara-release.keystore \
  -alias ocara \
  -keyalg RSA -keysize 2048 -validity 10000
```

`keytool` demande un mot de passe (garde-le) et quelques informations (nom, organisation…). **En dehors du dépôt** (ex: `~/.android-keystores/`, jamais dans `packaging/android/`) — un keystore commité par erreur, même dans un dépôt privé, doit être considéré compromis et régénéré. `-validity 10000` (~27 ans) : le Play Store exige que le certificat de signature reste valide au moins jusqu'en 2033 pour toute nouvelle app.

### 5.2 Définir les variables d'environnement

```bash
export OCARA_RELEASE_KEYSTORE=~/.android-keystores/ocara-release.keystore
export OCARA_RELEASE_KEYSTORE_PASSWORD='<mot de passe du keystore>'
export OCARA_RELEASE_KEY_ALIAS=ocara
export OCARA_RELEASE_KEY_PASSWORD='<mot de passe de la clé — souvent le même>'
```

Lues par `app/build.gradle.kts` (`signingConfigs["release"]`) — **jamais écrites en dur dans un fichier commité**. Sans elles, `make android-production` échoue immédiatement avec un message clair plutôt que de laisser Gradle produire un APK non signé silencieusement.

### 5.3 Construire

Depuis `examples/advanced/mini_project/` (le patron s'applique à tout autre programme Ocara packagé de la même façon) :

```bash
make android-production
```

Compile le runtime + pont JNI (arm64 et x86_64, comme `make android`), lie le `.so` de l'exemple pour les deux ABI, puis `./gradlew assembleRelease` — signé, minifié (`isMinifyEnabled`/`isShrinkResources`, voir `app/build.gradle.kts`) via les règles de `app/proguard-rules.pro`. APK produit : `packaging/android/app/build/outputs/apk/release/app-release.apk`.

**Piège réel corrigé au passage** : `OcaraBridge.nativeStartServer` est résolu par le pont JNI natif via son nom Java **exact**, figé à la compilation du `.so` (`Java_com_ocara_bridge_OcaraBridge_nativeStartServer`) — sans règle R8 dédiée, la minification aurait pu renommer cette classe/méthode, cassant le chargement du `.so` au premier appel (`UnsatisfiedLinkError`), **uniquement en production** (le debug, jamais minifié, ne l'aurait jamais révélé). `app/proguard-rules.pro` la protège explicitement (`-keep`).

### Vérification

Signature réelle :

```bash
$ANDROID_HOME/build-tools/<version>/apksigner verify --print-certs \
  packaging/android/app/build/outputs/apk/release/app-release.apk
```

**Vérifié réellement** (keystore de test, à ne jamais réutiliser en production) : `assembleRelease` réussit (minification + réduction des ressources incluses), signature confirmée par `apksigner`, taille réduite d'environ 40 % par rapport au debug (~57 Mo contre ~96 Mo pour `examples/advanced/mini_project`, essentiellement les deux `.so` non compressibles). Installé sur un vrai téléphone (désinstallation de la version debug au préalable — signatures différentes, Android refuse une mise à jour qui change de signature) : chargement JNI, serveur HTTP, rendu HTML tous fonctionnels — `curl` répond `HTTP 200` avec le contenu attendu, confirmant que la règle R8 ci-dessus protège bien ce qu'il fallait.

---

## 6. Builtins natifs Android

Huit builtins, un par domaine, donnent accès aux capacités du téléphone hors
interface graphique. Chacun a son crate Rust (`runtime_android/<domaine>`),
lié dans le `.so` **seulement si le programme l'importe** :

| Builtin | Domaine | Voie |
|---------|---------|------|
| [AndroidPermissions](builtins/AndroidPermissions.md) | permissions d'exécution | JNI → `OcaraPermissions.kt` |
| [AndroidNotifications](builtins/AndroidNotifications.md) | notifications système | JNI → `OcaraNotifications.kt` |
| [AndroidSensors](builtins/AndroidSensors.md) | capteurs | NDK `ASensorManager` (sans Java) |
| [AndroidFiles](builtins/AndroidFiles.md) | répertoires, assets, Téléchargements | JNI → `OcaraFiles.kt` |
| [AndroidCamera](builtins/AndroidCamera.md) | photo | JNI → `OcaraCamera.kt` (Camera2) |
| [AndroidMicrophone](builtins/AndroidMicrophone.md) | enregistrement | JNI → `OcaraMicrophone.kt` (`MediaRecorder`) |
| [AndroidAudio](builtins/AndroidAudio.md) | lecture audio | JNI → `OcaraAudio.kt` (`MediaPlayer`) |
| [AndroidUI](builtins/AndroidUI.md) | interface : bouton retour et événements, barres système, orientation, plein écran, vibration, partage, boîtes de dialogue | JNI → `OcaraUI.kt` ; événements Kotlin → Ocara par `OcaraBridge.nativeEvent` |

**Fonctionnement.** Le pont JNI (`runtime_android_jni`) note la JVM et la
classe `OcaraBridge` au démarrage. Le crate commun
(`runtime_android/common`) attache le thread Ocara à la JVM au premier appel,
charge les classes de l'app par le chargeur d'`OcaraBridge` et appelle leurs
méthodes statiques ; une exception Java devient une `AndroidException`. Le
code Kotlin vit dans `packaging/android/app/src/main/java/com/ocara/bridge/`.

**Construire.** En plus du runtime et du pont (§1, §4) :

```bash
ANDROID_NDK_HOME="$NDK" make build-android-features   # runtime_android/*, à côté de libocara_runtime.a
```

`ocara ... --android-runtime <dossier>/libocara_runtime.a --android-jni-bridge ...`
trouve les archives des builtins importés dans le même dossier ;
`--android-jni-bridge` est obligatoire dès qu'un builtin `Android*` est
importé. Exemple complet : `examples/advanced/android_native/` (`make android`).

**Dans l'app.** `MainActivity` appelle `OcaraBridge.attach(this)` et
`OcaraUI.install(this)` avant `nativeStartServer`, `OcaraUI.release()` puis
`OcaraBridge.detach(this)` dans `onDestroy`, et transmet
`onRequestPermissionsResult` à `OcaraPermissions.onResult`. Le programme
Ocara ne démarre qu'une fois par processus (une Activity recréée ne le
relance pas) ; l'Activity déclare `android:configChanges` (orientation,
taille…) pour qu'une rotation ne la recrée pas.

**Événements Kotlin → Ocara.** `OcaraBridge.nativeEvent(event, data)` (pont
JNI) remet l'événement à `runtime_android/ui`, qui le traite sur un thread
Ocara dédié (`AndroidUI::listen`) : jamais sur le thread principal Android. Le manifeste
déclare les permissions des builtins utilisés (voir les commentaires de
`AndroidManifest.xml`). Un chemin relatif part de `filesDir`, répertoire de
travail du programme.

**Hors Android**, un programme qui importe ces builtins compile ; chaque
appel lève `AndroidException` 100.

**Vérifié** sur un téléphone réel (arm64-v8a, Android 15) : demande de
permission avec la boîte de dialogue système, notification, relevé de 8
capteurs, répertoires et assets, photo 3120×4160 correctement exposée,
enregistrement et relecture au micro, copie vers Téléchargements ; pour
`AndroidUI` : bouton retour intercepté (confirmation, annulation, fermeture),
message bref, vibration, style des barres, plein écran, paysage sans
recréation de l'Activity, alerte, partage.

### Codes d'erreur AndroidException

| Code | Cause |
|------|-------|
| 100 | Hors Android, ou programme non démarré par `OcaraBridge` |
| 101 | Permission non accordée |
| 102 | Argument invalide (nom de permission ou de capteur inconnu, valeur hors bornes) |
| 103 | Matériel absent ou indisponible (capteur, caméra, micro, stockage externe) |
| 104 | Fichier ou asset introuvable, lecture ou écriture impossible |
| 105 | Échec Android (autre exception Java) |
| 106 | Délai dépassé (relevé de capteur, photo) |
| 107 | État invalide (appel bloquant sur le thread principal Android, `stop` sans `start`…) |

---

## Limitations connues

- **`is_pic` n'est activé que pour une cible croisée** (`--target` explicite) — le chemin de compilation normal (hôte) reste inchangé (`-no-pie`, pas de PIC). Voir [roadmap.d/securite-pie-cranelift-is-pic.md](roadmap.d/securite-pie-cranelift-is-pic.md).
- **`libz-sys` lie `libz` dynamiquement sur Android**, contrairement à OpenSSL (statique y compris pour Android) — son propre `build.rs` part du principe que tout compilateur Android est livré avec `libz`, ce qui est vrai sur toutes les versions d'Android testées par ce projet en amont.
- **`ocara.SDL` sur Android n'a pas les formats audio "tracker"/MOD** (bug d'édition de liens en amont, voir ci-dessus) — WAV/OGG/MP3/FLAC/Opus fonctionnent normalement.
- **Pointeurs de tas Android tagués (Scudo)** — l'allocateur natif d'Android peut renvoyer des pointeurs dont l'octet de poids fort est non nul (voir [roadmap.d/runtime-android-aarch64-pic-string-concat.md](roadmap.d/runtime-android-aarch64-pic-string-concat.md)) ; corrigé en désactivant ce tagging pour le processus (`mallopt` dans le pont JNI), vérifié en exécution réelle.
- **`ocara.Tauri` sur une cible Android** ne rejette plus la compilation : compilé en talon local no-op (voir [roadmap.d/packaging-android-webview-hybrid.md](roadmap.d/packaging-android-webview-hybrid.md)) — un simple avertissement est émis à la place, ce qui permet un point d'entrée `.oc` unique partagé entre desktop et Android.
- **`ocara.Tauri`** ne fonctionne pas et ne fonctionnera jamais sur Android (GTK, voir tableau ci-dessus).
- **Packaging APK** : un squelette Gradle minimal existe (`packaging/android/`, WebView + pont JNI, voir §4) et produit un APK réel, testé avec succès sur un émulateur ET sur un appareil physique (`examples/advanced/mini_project` fonctionne bout en bout, à l'exception du bug de rendu ci-dessus) — mais reste un squelette de démonstration (port/nom de `.so`/package codés en dur, pas de cycle de vie Android), pas encore un gabarit générique réutilisable pour n'importe quel programme Ocara. Fichiers statiques (CSS, images…) : à copier manuellement dans `packaging/android/app/src/main/assets/` avant `./gradlew assembleDebug` — Gradle ne le fait pas automatiquement pour un projet Ocara, voir `packaging/android/README.md`. Voir la stratégie envisagée dans [roadmap.d/packaging-android.md](roadmap.d/packaging-android.md), et les chantiers suivants [webview hybride](roadmap.d/packaging-android-webview-hybrid.md) / [interface native `ocara.UI`](roadmap.d/langage-builtin-ui-multiplateforme.md).
