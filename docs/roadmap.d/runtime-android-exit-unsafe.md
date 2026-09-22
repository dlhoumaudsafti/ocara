# `__ocara_fail` (sortie sur exception non gérée) fait planter le processus sur Android — testé sur un vrai appareil

## Constat

Découvert en testant `examples/advanced/mini_project/main_android.oc` sur un **vrai appareil Android** (Realme RMX3834, Android 15/API 35, arm64-v8a — voir [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md)), après que le même code a fonctionné correctement sur un émulateur x86_64 (Android API 36, image AOSP standard).

Sur l'appareil réel, `Database::connect()` → `SQLite::open(...)` échoue (raison exacte non déterminée, voir plus bas) et déclenche `throw_sqlite_exception`. Comme rien ne catch cette exception, elle atteint `__ocara_fail`, qui appelle `std::process::exit()` — et LÀ, le processus **plante violemment** (`SIGABRT`/`SIGSEGV`) au lieu de terminer proprement, avec deux variantes observées (tombstones réels, pas une supposition) :

```
Fatal signal 6 (SIGABRT) ...
#04 art::Mutex::~Mutex()
#05 __cxa_finalize
#06 exit
#07 std::sys::exit::exit
#08 std::process::exit
#09 __ocara_fail
#10 ocara_runtime::exception::throw_sqlite_exception
#11 SQLite_open
#12 Database_connect
```

et, sur une autre tentative (même point d'origine, destructeur différent) :

```
Fatal signal 6 (SIGABRT) ...
#04 android::uirenderer::CommonPool::~CommonPool()
#05 __cxa_finalize
#06 exit
#07 std::sys::exit::exit
#08 std::process::exit
#09 __ocara_fail
#10 ocara_runtime::exception::throw_sqlite_exception
#11 SQLite_open
#12 Database_connect
```

**Le point commun, dans les deux cas** : `std::process::exit()` déclenche `__cxa_finalize`, qui exécute les destructeurs de TOUS les objets globaux du processus — y compris ceux d'ART (`art::Mutex`) et de composants système Android chargés dans le même processus (`libhwui.so`, le moteur de rendu graphique). Ces objets n'ont jamais été conçus pour être détruits "en cours de vie" d'une app Android — leur destruction déclenche un état invalide qu'ART/Bionic détectent explicitement et transforment en `abort()` fatal, contrairement à un process Linux normal où `exit()` est sûr et attendu (c'est exactement le patron déjà rencontré une fois, en creusant [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) §"Deux bugs réels", avant la découverte du chemin SQLite — la destruction de mutex qu'on croyait résolue par le contournement de chemin ne l'était pas : elle est **systématique**, dès que `__ocara_fail` s'exécute sur Android, quelle que soit la cause de l'exception).

## Cause

`__ocara_fail` (`runtime/src/lib.rs:3149`, le point d'arrivée de toute exception Ocara non gérée quand aucun `try`/`on` actif ne l'attrape — `TRY_STACK` vide) écrit son message d'erreur en `stderr` brut (`write_stderr_raw`, ligne 3202) puis appelle inconditionnellement `std::process::exit(1)` (ligne 3203) — un choix sûr et correct sur desktop (Linux/macOS/Windows, où le processus Ocara EST le processus entier), mais catastrophique sur Android, où le programme Ocara ne tourne que dans un THREAD d'un processus d'app bien plus large, partagé avec la JVM/ART et tous les composants système Android déjà chargés dans ce même processus. Le message d'erreur lui-même (`write_stderr_raw`) n'apparaît nulle part dans `logcat` sur Android — un `.so` chargé par JNI n'a pas son `stderr` connecté au système de log, contrairement à un processus Linux normal lancé depuis un terminal.

## Ce qui reste une inconnue réelle (pas juste non creusée par manque de temps)

**La cause exacte de l'échec `SQLite::open()` sur ce téléphone précis n'est PAS déterminée.** Le chemin utilisé (`/data/user/0/com.ocara.demo/app.db`) a un propriétaire et un contexte SELinux corrects et standards (`u:object_r:app_data_file:s0:...`, vérifié via `adb shell run-as ... ls -laZ`) — rien d'anormal à l'inspection. Piste éliminée : la taille de page mémoire (`getconf PAGE_SIZE` confirme 4096, pas le nouveau standard 16 KB qui casse certaines bibliothèques natives non recompilées pour ça). **Aucune autre piste concrète à ce stade.**

Le blocage réel pour aller plus loin : **aucune visibilité sur les messages d'erreur du runtime Ocara lui-même sur Android.** `IO::writeln`/le message exact de `throw_sqlite_exception` n'apparaissent nulle part dans `logcat` — contrairement à un programme Linux normal, le `stdout` d'un `.so` chargé par une app Android n'est pas capturé par le système de log (seul `__android_log_print` l'est, jamais utilisé par le runtime Ocara aujourd'hui). Deviner d'autres chemins/permissions sans ce retour serait rejouer la même méthode par tâtonnement déjà insuffisante ici plutôt que de résoudre le vrai manque.

## Priorité / Complexité

**Priorité Très Basse** — bloque uniquement le test sur appareil réel du chantier [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) (déjà Très Basse), qui fonctionne par ailleurs sur émulateur. **Complexité non évaluée** : le correctif du symptôme immédiat (rendre `__ocara_fail` sûr sur Android — ne pas appeler `process::exit()`, ou l'appeler d'une façon qui ne déclenche pas `__cxa_finalize` sur les globales du processus, ex. `libc::_exit()`/`std::process::abort()` sans unwind plutôt que `exit()`) semble Léger une fois identifié précisément ; la cause racine de l'échec SQLite reste, elle, non évaluée tant qu'aucune visibilité de log n'existe (voir ci-dessus — probablement le premier prérequis avant de pouvoir diagnostiquer quoi que ce soit d'autre sur Android à l'avenir, pas seulement ce bug-ci).

## Fichiers clés

`runtime/src/lib.rs:3149` (`__ocara_fail`, `std::process::exit(1)` ligne 3203), `runtime/src/exception.rs` (contexte des exceptions, appelants de `__ocara_fail`), `examples/advanced/mini_project/configs/Database.oc` (le déclencheur observé, pas la cause), [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) (contexte complet du test réel), [packaging-android-native-features](packaging-android-native-features.md) (pourrait être le bon endroit pour un futur builtin de logging Android via `__android_log_print`, prérequis pour diagnostiquer ce genre de problème correctement).
