# Builtins natifs Android : suite

Statut : **base livrée** (2026-10-27), priorité très basse pour la suite.
Voir [docs/android.md](../android.md) §6 pour ce qui existe.

## Existant (vérifié)

- Un builtin par domaine, un crate Rust par builtin
  (`runtime_android/<domaine>`), lié dans le `.so` seulement si le programme
  l'importe (`android_feature_libs`, `src/codegen/link.rs`) :
  `ocara.AndroidPermissions`, `AndroidNotifications`, `AndroidSensors`,
  `AndroidFiles`, `AndroidCamera`, `AndroidMicrophone`, `AndroidAudio`, et
  une `AndroidException` commune (codes 100 à 107).
- Socle commun `runtime_android/common` (rlib) : appel du runtime Ocara en ABI
  C, attachement du thread Ocara à la JVM, chargement des classes de l'app par
  le chargeur d'`OcaraBridge` (JVM et classe notées par le pont JNI,
  `runtime_android_jni`), exception Java → `AndroidException`.
- Côté Kotlin : `packaging/android/app/src/main/java/com/ocara/bridge/Ocara*.kt`
  (Camera2, `MediaRecorder`, `MediaPlayer`, `MediaStore`, `NotificationManager`,
  `requestPermissions`). Les capteurs passent par le NDK `ASensorManager`
  (`registerSensor` résolu à l'exécution : le `.so` se charge dès Android 7).
- Hors Android, chaque appel lève `AndroidException` 100 (talon local,
  `src/codegen/emit.d/emitter.rs`) : une même source sert desktop et Android.
- Vérifié sur un téléphone réel (arm64-v8a, Android 15) avec
  `examples/advanced/android_native` : boîte de dialogue de permission et
  réponse, notification, 8 capteurs, répertoires et assets, photo
  3120×4160 exposée, enregistrement et relecture, copie vers Téléchargements.
- Tests : `examples/tests/119_android_builtins_desktopTest.oc` (comportement
  hors Android), test de sélection des archives (`src/codegen/link.rs`).

## Reste à faire

- **Tests sur appareil automatisés** : la vérification sur téléphone est
  manuelle (`curl` via `adb forward` sur les routes de l'exemple). Un
  `make tests-android` qui installe l'exemple et interroge chaque route
  (permissions accordées d'avance par `adb shell pm grant`) éviterait les
  régressions silencieuses. L'émulateur x86_64 n'a pas été repassé pour ces
  builtins.
- **Relevés continus des capteurs** : `read` rend un seul relevé ; un flux
  (`message<array<float>>`, ou un rappel) éviterait de recréer la file
  d'événements à chaque lecture.
- **Localisation** : alias de permission `location` présent, aucun builtin
  (`LocationManager`/fused location, relevé ponctuel puis continu).
- **Caméra** : aperçu affiché, vidéo, choix de la résolution ; la photo prend
  aujourd'hui la plus grande résolution JPEG.
- **Notifications** : actions, importance, image, notification planifiée.
- **Fichiers** : sélecteur de documents (Storage Access Framework), qui
  demande un retour d'Activity comme les permissions.
- **Micro et audio** : niveau sonore et PCM brut (AAudio), plusieurs sons
  simultanés.
- **Cycle de vie** : un `AndroidPermissions::request` en attente le reste si
  l'Activity est détruite avant la réponse ; aucune explication
  (`shouldShowRequestPermissionRationale`) ni renvoi vers les réglages.
- **Manifeste** : les permissions sont à déclarer à la main ; l'outil de
  packaging pourrait les déduire des imports.

## Complexité

Légère (notifications, fichiers, localisation ponctuelle) à Structurel
(relevés continus, tests sur appareil, sélecteur de documents, vidéo).
