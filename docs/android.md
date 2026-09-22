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
| Packaging APK / test sur appareil réel | ❌ Non fait |

Tout ce qui est vérifiable **par inspection statique du binaire produit** (architecture ELF correcte, bibliothèque réellement partagée, absence de `DT_TEXTREL`, toutes les dépendances dynamiques déclarées et effectivement fournies par les bibliothèques système du NDK) l'a été. Le comportement réel au chargement sur un appareil/émulateur (JNI, `System.loadLibrary`) n'a **pas** été testé — cet environnement n'a pas d'appareil/émulateur Android disponible.

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

---

## Limitations connues

- **`is_pic` n'est activé que pour une cible croisée** (`--target` explicite) — le chemin de compilation normal (hôte) reste inchangé (`-no-pie`, pas de PIC). Voir [roadmap.d/securite-pie-cranelift-is-pic.md](roadmap.d/securite-pie-cranelift-is-pic.md).
- **`libz-sys` lie `libz` dynamiquement sur Android**, contrairement à OpenSSL (statique y compris pour Android) — son propre `build.rs` part du principe que tout compilateur Android est livré avec `libz`, ce qui est vrai sur toutes les versions d'Android testées par ce projet en amont.
- **`ocara.SDL` sur Android n'a pas les formats audio "tracker"/MOD** (bug d'édition de liens en amont, voir ci-dessus) — WAV/OGG/MP3/FLAC/Opus fonctionnent normalement.
- **Aucun test sur appareil ou émulateur réel** — seule l'inspection statique du binaire a été faite (architecture ELF, absence de `DT_TEXTREL`, dépendances dynamiques résolues et réellement exportées).
- **`ocara.Tauri`** ne fonctionne pas et ne fonctionnera jamais sur Android (GTK, voir tableau ci-dessus).
- **Packaging APK** : un squelette Gradle minimal existe (`packaging/android/`, WebView + pont JNI, voir §4) et produit un APK réel qui embarque un `.so` Ocara — mais reste un squelette de démonstration (port/nom de `.so` codés en dur, pas de cycle de vie Android), pas encore un gabarit générique réutilisable pour n'importe quel programme Ocara. Voir la stratégie envisagée dans [roadmap.d/packaging-android.md](roadmap.d/packaging-android.md), et les chantiers suivants [webview hybride](roadmap.d/packaging-android-webview-hybrid.md) / [GUI native](roadmap.d/packaging-android-gui-native.md).
