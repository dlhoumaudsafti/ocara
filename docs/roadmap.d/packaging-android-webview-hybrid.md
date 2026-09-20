# Applications Android hybrides (WebView + serveur HTTP Ocara embarqué)

## Idée

Rendre un programme Ocara du style [examples/advanced/mini_project](../../examples/advanced/mini_project) (serveur `ocara.HTTPServer` + contrôleurs/modèles + templates HTML rendus côté serveur) **fonctionnel sur Android**, en l'affichant dans une `WebView` Android système pointée sur le serveur HTTP embarqué (`http://127.0.0.1:<port>/`) — même principe que Cordova/Capacitor, ou que ce que Tauri fait sur desktop (une fenêtre native + un moteur web, mais le "backend" est du code Ocara natif, pas du JS).

C'est un point de départ délibérément plus étroit et plus sûr que le support SDL/GUI native (voir [packaging-android](packaging-android.md) sous-chantier 4 et le ticket suivant sur la GUI native) : **aucune dépendance à SDL3, GTK ou Tauri** — un serveur HTTP est juste des sockets, déjà vérifié comme cross-compilable pour Bionic ([packaging-android](packaging-android.md) sous-chantier 3, `ocara_runtime` de base). Le morceau réellement nouveau ici est le pont JNI + le squelette Android (Activity/WebView/Gradle), pas le runtime Ocara lui-même.

## Ce qui est déjà en place (vérifié par [packaging-android](packaging-android.md))

- Cross-compilation Cranelift vers `aarch64-linux-android` (sous-chantier 1).
- `ocara_runtime` (donc `ocara.HTTPServer`, sockets, SQLite, HTML) compile pour Bionic (sous-chantier 3).
- Liaison en `.so` partagé, PIC, sans `DT_TEXTREL`, chargeable par le dynamic linker Bionic (sous-chantier 2).
- Un `.so` Ocara exporte déjà `main`/`__fn_wrap_main` comme symboles ordinaires (confirmé par `objdump -t` lors de la vérification du sous-chantier 2) — un point d'entrée qu'un pont JNI peut appeler directement, sans modification du compilateur.

## Ce qui manque, non vérifié

1. **Pont JNI** : le `.so` produit par `ocara` n'a aujourd'hui aucune notion de JNI (`JNI_OnLoad`, conventions d'appel `JNIEnv*`/`jobject`). Piste envisagée : un petit crate Rust séparé (même patron que `runtime_tauri`/`runtime_sdl` — un `.a`/`.so` à part, lié uniquement quand une cible Android avec ce mode est demandée) qui expose `Java_<package>_<Classe>_nativeStart`, appelle `main()` du `.so` Ocara sur un thread dédié, et signale au code Java/Kotlin quand le serveur écoute (pour ne pas charger la `WebView` avant que le port réponde). Non commencé.
2. **Squelette Android (Activity + Gradle)** : une `Activity` minimale avec une `WebView` chargeant `http://127.0.0.1:<port>/` après le démarrage du serveur natif. Rejoint la stratégie de packaging "gabarit + injection" déjà envisagée dans [packaging-android](packaging-android.md) (dépose du `.so` dans `jniLibs/`) — mais ici sans avoir besoin du template Android de SDL, un projet Gradle minimal "WebView + un `.so`" suffit probablement.
3. **Cleartext HTTP local** : Android ≥ 9 (API 28) bloque par défaut le trafic HTTP en clair pour une `WebView`, sauf configuration explicite (`android:usesCleartextTraffic` ou `network_security_config.xml`) ou exemption déjà accordée à `127.0.0.1`/`localhost` sur certaines versions — **non vérifié**, à confirmer avant de considérer le port HTTP local comme acceptable tel quel (alternative si bloqué : servir en HTTPS local avec un certificat auto-signé + exception, ou passer par un `WebViewAssetLoader`/schéma personnalisé — pistes non explorées).
4. **Permission réseau** (`INTERNET` dans le manifeste) même pour une boucle locale — comportement Android standard, pas une inconnue, juste à ne pas oublier.
5. **Cycle de vie Android** (pause/reprise de l'Activity, arrêt du serveur natif à la fermeture) — non réfléchi, l'exemple `mini_project` actuel tourne en processus unique sans notion de cycle de vie.

## Priorité / Complexité

**Priorité Très Basse** — étape "pour commencer" explicitement voulue par David avant la GUI native, mais aucune urgence : aucun besoin concret aujourd'hui, dépend entièrement du sous-chantier Android déjà *(Massif)*. **Complexité Massive** — nouveau pont JNI (zone jamais touchée par ce projet), nouveau squelette Gradle/Android (outillage jamais utilisé par ce projet non plus, SDK Android + Gradle + JDK à installer), plusieurs inconnues non tranchables par simple lecture de code (cleartext HTTP, cycle de vie).

## Fichiers clés

[packaging-android](packaging-android.md) (prérequis, déjà fait pour les sous-chantiers 1-3), `examples/advanced/mini_project/` (l'exemple de référence à faire tourner sur Android), `src/codegen/link.rs` (`link_android`, à étendre ou dupliquer pour lier ce pont JNI en plus du runtime), aucun fichier Android/Gradle n'existe encore dans ce dépôt.
