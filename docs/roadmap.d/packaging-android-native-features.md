# Fonctionnalités natives Android (hors GUI) : notifications, capteurs, fichiers, caméra, micro, audio, permissions

## Idée

Au-delà de l'affichage (WebView hybride ou GUI native, voir les tickets voisins), permettre à un programme Ocara compilé pour Android d'accéder aux capacités natives de la plateforme, **indépendamment de tout choix d'UI** :

- Notifications système (`NotificationManager`)
- Capteurs (accéléromètre, gyroscope, luminosité, etc. — l'API bas niveau NDK `ASensorManager` existe déjà et est déjà liée par [le sous-chantier SDL](packaging-android.md) via `-landroid`, mais aucun builtin Ocara ne l'expose)
- Accès aux fichiers (stockage de l'app, stockage partagé/`MediaStore`, Storage Access Framework selon la version d'Android visée)
- Accès caméra
- Accès microphone
- Gestion audio (lecture/enregistrement — au-delà de ce que `ocara.SDL`/`SDL_mixer` couvre déjà pour le jeu ; `AAudio`/`OpenSL ES` sont déjà liés pour SDL, mais l'accès direct au micro et à l'enregistrement n'est pas couvert par SDL_mixer)
- Gestion des permissions runtime Android (`Activity.requestPermissions`/résultat asynchrone — obligatoire pour caméra/micro/capteurs sensibles/notifications sur les versions récentes d'Android)

**Explicitement demandé par David** : chaque domaine doit vivre dans **son propre crate Rust séparé**, pas un unique gros crate fourre-tout — même principe déjà appliqué à `runtime_tauri`/`runtime_sdl` (voir leur doc dans `src/codegen/link.rs`) : un crate séparé par domaine donne une frontière de lien dure (un programme qui n'importe pas `ocara.AndroidCamera` ne traîne aucun code caméra, aucune permission caméra implicite dans son `.so`) et une meilleure lisibilité (chaque crate reste petit et concentré sur un seul domaine, plutôt qu'un `runtime_android` monolithique).

## Pourquoi c'est un chantier à part, pas une extension triviale du reste

La quasi-totalité de ces domaines n'ont **aucune API NDK (C) directe** — contrairement à `ASensorManager`/`AAsset*`/`ANativeWindow` (déjà utilisés par SDL, purement NDK) :

- Notifications, permissions runtime, et la plupart des accès fichiers "modernes" (Storage Access Framework, `MediaStore`) sont des API **Java/Kotlin uniquement**, sans équivalent NDK — un pont JNI vers ces classes Android (`android.app.NotificationManager`, `android.app.Activity.requestPermissions`, `android.provider.MediaStore`...) est un préalable obligatoire pour chacune.
- Caméra a un NDK bas niveau (`libcamera2ndk`) mais son API est nettement plus complexe que Camera2/CameraX côté Java — à évaluer laquelle des deux voies est la plus raisonnable une fois ce chantier commencé.
- Micro/audio ont un chemin NDK direct (`AAudio`, déjà entrevu via `OpenSLES` pour SDL) mais l'enregistrement (pas seulement la lecture) et la gestion des permissions micro associée restent à vérifier.

Ce chantier dépend donc du **pont JNI générique** déjà identifié comme prérequis dans [le hybride WebView](packaging-android-webview-hybrid.md) (`Java_<package>_<Classe>_native*`) — probablement le même mécanisme de base, étendu domaine par domaine plutôt que construit sept fois indépendamment.

## Périmètre par domaine (aucun commencé, aucune piste tranchée)

| Domaine | Crate envisagé | Voie API |
|---|---|---|
| Notifications | `runtime_android_notifications` | JNI → `NotificationManager` (Java uniquement) |
| Capteurs | `runtime_android_sensors` | NDK `ASensorManager` (déjà lié pour SDL, jamais exposé comme builtin) |
| Fichiers | `runtime_android_files` | NDK `AAssetManager` (assets bundlés) + JNI (stockage partagé/SAF selon la cible) |
| Caméra | `runtime_android_camera` | NDK `libcamera2ndk` ou JNI Camera2/CameraX — à trancher |
| Microphone | `runtime_android_microphone` | NDK `AAudio`/`OpenSL ES` (enregistrement, à vérifier) |
| Audio (lecture/gestion) | `runtime_android_audio` | NDK `AAudio`/`OpenSL ES` |
| Permissions | `runtime_android_permissions` | JNI → `Activity.requestPermissions` (Java uniquement, résultat asynchrone) |

Chaque crate exposerait un builtin Ocara dédié (ex. `ocara.AndroidNotifications`, `ocara.AndroidCamera`...) suivant la même convention `import ocara.<Nom>` que les builtins existants (voir `docs/compilation-guide.md` §6) — noms exacts non tranchés.

## Priorité / Complexité

**Priorité Très Basse** — dépend entièrement de [packaging-android](packaging-android.md) (fait) et du pont JNI du [hybride WebView](packaging-android-webview-hybrid.md) (pas fait), aucun besoin concret aujourd'hui. **Complexité Massive** — sept domaines indépendants, la plupart nécessitant une intégration JNI Java/Kotlin jamais faite dans ce projet, plus la gestion asynchrone des permissions runtime (modèle de callback à réconcilier avec le modèle synchrone/à threads d'Ocara).

## Fichiers clés

Aucun — chantier pas commencé. [packaging-android](packaging-android.md) (infrastructure de cross-compilation, prérequis, fait), [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) (pont JNI générique, prérequis probable, pas fait), `src/codegen/link.rs` (patron `needs_tauri`/`needs_sdl` à étendre à sept nouveaux domaines conditionnels), `docs/builtins/` (convention de documentation des builtins existants, à suivre pour chaque nouveau domaine).
