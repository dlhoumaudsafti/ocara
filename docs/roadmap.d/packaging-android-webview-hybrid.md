# Applications Android hybrides (WebView + serveur HTTP Ocara embarqué)

## Fondations — Fait, vérifié (pont JNI + squelette Gradle, APK réel produit)

Voir "Ce qui manque" ci-dessous, points 1 et 2 : **faits et vérifiés**. Le reste (points 3-5 : cleartext HTTP, permission, cycle de vie) est traité de façon minimale mais fonctionnelle pour ce squelette — voir le détail plus bas.

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

## Ce qui reste (hors "fondations")

- Adapter `examples/advanced/mini_project/main.oc` : importe `ocara.Tauri` (fenêtre desktop), **incompatible tel quel** avec Android (`link_android` le rejette) — une variante Android de son point d'entrée (juste `Server::start()`, sans fenêtre) reste à écrire.
- Cycle de vie Android (voir point 5 ci-dessus).
- Test réel sur appareil/émulateur — non tenté (`android emulator` existe dans le CLI découvert ci-dessus, piste à explorer si un besoin de vérification plus poussée se présente, non exploré ici faute de temps/scope).
- Généralisation en builtin `ocara.UIHybrid`, voir [langage-builtin-ui-hybride](langage-builtin-ui-hybride.md) — tout ici reste câblé à la main (port et nom de `.so` codés en dur), pas une abstraction du compilateur.

## Priorité / Complexité

**Priorité Très Basse** — étape "pour commencer" explicitement voulue par David avant la GUI native, fondations désormais posées. **Complexité** : le pont JNI et le squelette Gradle, une fois les bons outils trouvés (le CLI `android`), se sont avérés une **addition contenue** plutôt qu'un chantier ouvert — les vraies inconnues (extraction paresseuse d'archive statique, cleartext HTTP) ont chacune une réponse simple une fois identifiées.

## Fichiers clés

`runtime_android_jni/` (pont JNI, **fait**), `src/codegen/link.rs` (`link_android`, paramètre `jni_bridge_lib` + `-Wl,-u`, **fait**), `src/core/cli.rs` (flag `--android-jni-bridge`, **fait**), `Makefile` (cible `build-jni-bridge-android`, **fait**), `packaging/android/` (squelette Gradle complet, **fait** — voir son propre `README.md`), [packaging-android](packaging-android.md) (prérequis, les 4 sous-chantiers sont faits), `examples/advanced/mini_project/` (l'exemple de référence, PAS ENCORE adapté pour Android), [langage-builtin-ui-hybride](langage-builtin-ui-hybride.md) (généralisation en builtin Ocara, pas commencée).
