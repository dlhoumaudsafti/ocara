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

| Outil | Où l'obtenir | Rôle |
|-------|--------------|------|
| NDK Android (r23+, tout-LLVM) | [developer.android.com/ndk/downloads](https://developer.android.com/ndk/downloads) | clang croisé + sysroot Bionic |
| Cible Rust `aarch64-linux-android` | `rustup target add aarch64-linux-android` | Compiler `ocara_runtime` pour Bionic |

`ocara` lui-même **n'a pas besoin** du NDK pour être construit (`make build` reste inchangé) — le NDK ne sert qu'à (a) cross-compiler le runtime Ocara une fois, et (b) lier le programme final.

Seule l'ABI `arm64-v8a` (triple `aarch64-linux-android`) a été vérifiée — de loin la plus pertinente aujourd'hui (quasi tous les appareils Android actuels). `armv7-linux-androideabi`/`x86_64-linux-android`/`i686-linux-android` sont reconnus par le CLI mais **non testés**.

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

## Limitations connues

- **`is_pic` n'est activé que pour une cible croisée** (`--target` explicite) — le chemin de compilation normal (hôte) reste inchangé (`-no-pie`, pas de PIC). Voir [roadmap.d/securite-pie-cranelift-is-pic.md](roadmap.d/securite-pie-cranelift-is-pic.md).
- **`libz-sys` lie `libz` dynamiquement sur Android**, contrairement à OpenSSL (statique y compris pour Android) — son propre `build.rs` part du principe que tout compilateur Android est livré avec `libz`, ce qui est vrai sur toutes les versions d'Android testées par ce projet en amont.
- **`ocara.SDL` sur Android n'a pas les formats audio "tracker"/MOD** (bug d'édition de liens en amont, voir ci-dessus) — WAV/OGG/MP3/FLAC/Opus fonctionnent normalement.
- **Aucun test sur appareil ou émulateur réel** — seule l'inspection statique du binaire a été faite (architecture ELF, absence de `DT_TEXTREL`, dépendances dynamiques résolues et réellement exportées).
- **`ocara.Tauri`** ne fonctionne pas et ne fonctionnera jamais sur Android (GTK, voir tableau ci-dessus).
- **Packaging APK** (gabarit Gradle, injection du/des `.so` dans `jniLibs/`) non fait — voir la stratégie envisagée dans [roadmap.d/packaging-android.md](roadmap.d/packaging-android.md), et les chantiers suivants [webview hybride](roadmap.d/packaging-android-webview-hybrid.md) / [GUI native](roadmap.d/packaging-android-gui-native.md).
