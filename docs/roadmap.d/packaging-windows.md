# Support Windows pour la compilation du compilateur — établi (runtime de base), Tauri/SDL restants

## Fait, vérifié — `ocara`/`ocara_runtime` cross-compilés pour Windows, testés bout en bout via Wine

Contrairement au constat initial ci-dessous (conservé tel quel comme trace), **`ocara` lui-même a maintenant été construit et testé sur Windows** (cross-compilé depuis Linux via `x86_64-w64-mingw32-gcc`, exécuté et vérifié sous Wine — pas une supposition). Voir [docs/windows.md](../windows.md) pour la procédure complète et §"Ce qui a été fait" ci-dessous pour le détail des corrections.

## Constat initial (avant ce chantier)

Aucune preuve que le compilateur `ocara` lui-même ait jamais été construit ou testé sur Windows : pas de script `.bat`/PowerShell, le `Makefile` utilise des symlinks Unix (`ln -sf`), `runtime/src/mutex.rs` s'appuyait sur `pthread_mutex_t` (libc) sans fallback Windows (voir aussi [memoire-fiabilite-runtime-bas-niveau](memoire-fiabilite-runtime-bas-niveau.md)). `WebView2` n'était cité dans le `README.md` que comme runtime pour exécuter un programme Tauri déjà compilé sur Linux/macOS — pas comme cible de build du compilateur.

Contrainte notée à l'ouverture du ticket : cet environnement est Linux — la seule solution pour vérifier réellement un binaire Windows sans machine Windows physique est de le faire tourner sous Wine.

## Ce qui a été fait

### 1. Toolchain — `gcc-mingw-w64-x86-64` (64 bits UNIQUEMENT, 32 bits explicitement hors périmètre) + `rustup target add x86_64-pc-windows-gnu`

Installés manuellement (paquet Ubuntu `gcc-mingw-w64-x86-64` — PAS `mingw-w64` générique, qui inclurait aussi le 32 bits `i686-*`, jamais visé). Sanity check : un `fn main() { println!(...) }` trivial, cross-compilé et exécuté sous Wine — confirme que la chaîne complète (rustc → mingw-w64 → Wine) fonctionne avant de toucher au vrai projet.

### 2. `runtime/src/mutex.rs` — `pthread_mutex_*` n'existe pas dans `libc` pour Windows

Confirmé par échec de compilation réel (pas une supposition) : `libc` n'expose AUCUNE fonction `pthread_mutex_init/lock/unlock/trylock` pour une cible `*-windows-*` (Windows n'a pas de pthreads natif ; MinGW-w64 fournit bien `winpthreads`, mais le crate `libc` ne le lie pas). Corrigé avec un module `platform` interne (`#[cfg(unix)]`/`#[cfg(windows)]`) : `pthread_mutex_t` sur Unix (inchangé), `CRITICAL_SECTION` sur Windows (`windows-sys`, bindings Win32 officiels et légers) — mêmes opérations manuelles (init/lock/unlock/trylock/destroy, SANS RAII, nécessaire pour l'API `ocara.Mutex`) des deux côtés. `TryEnterCriticalSection` a une convention de retour OPPOSÉE à `pthread_mutex_trylock` (différent de zéro = succès, pas zéro) — normalisée en `bool` dans le module pour que l'appelant commun n'ait pas à le savoir.

**Deuxième site trouvé en creusant plus loin** : `runtime/src/lib.rs` (`__alloc_locked_cell`/`__locked_cell_get`/`__locked_cell_set`, le mutex interne des cellules de capture de closure — jamais exposé à Ocara) avait sa PROPRE redéclaration de `pthread_mutex_init/lock/unlock` via un `extern "C"` brut, avec un type `[u8; 40]`/`[u8; 64]` hardcodé par plateforme — le même risque déjà documenté (et déjà corrigé) dans `mutex.rs` avant que ce type utilise `libc::pthread_mutex_t` directement, mais jamais appliqué à ce deuxième site. Corrigé en réutilisant directement le module `platform` de `mutex.rs` (rendu `pub(crate)`) — une seule implémentation, plus de tableau d'octets hardcodé nulle part dans le runtime.

### 3. `build.rs` (racine) — n'était PAS conscient de la cible lors d'une cross-compilation

`ocara` embarque ses runtimes (`libocara_runtime.a`/`_tauri`/`_sdl`) via `include_bytes!` depuis `OUT_DIR`, copiés là par `build.rs` depuis `target/release/` — **en dur**, jamais `target/<triple>/release/`. Cross-compiler `ocara` lui-même (`--target x86_64-pc-windows-gnu`) aurait donc silencieusement embarqué le runtime de l'HÔTE (ELF Linux) au lieu de celui cross-compilé (PE Windows) — trouvé en lisant le code AVANT de lancer la compilation, pas après un échec mystérieux. Corrigé avec `target_release_dir()` : compare `TARGET`/`HOST` (toujours fournis par Cargo à un build script) pour choisir le bon répertoire, y compris pour le déclenchement automatique de compilation (`ensure_runtime_lib`) et les directives `cargo:rerun-if-changed` (celles-ci pointaient AUSSI en dur vers `target/release/` — un `ocara.exe` déjà construit ne se relinkait jamais quand seul le runtime cross-compilé changeait, confirmé par reproduction : `mtime` de `ocara.exe` antérieur à celui du runtime qu'il était censé embarquer).

`ocara_runtime_tauri` (WebView2 exige l'ABI MSVC — son import lib officielle est un `.lib` COFF/MSVC, illisible par `ar`/`ld` GNU, voir §"Ce qui reste") et `ocara_runtime_sdl` (pas encore essayé pour cette cible) ne sont pas encore disponibles pour Windows GNU : `build.rs` écrit une archive `ar` vide (juste l'en-tête magique `!<arch>\n`, aucun membre) à leur place pour cette cible — satisfait `include_bytes!` sans prétendre à un vrai support. Un programme qui importerait `ocara.Tauri`/`ocara.SDL` compilé par un tel `ocara.exe` échouerait au lien final (symboles non résolus) — un échec honnête, jamais un binaire cassé silencieusement.

### 4. `src/codegen/link.rs` — `link()` (le lien final "hôte", pas la liaison croisée Android) était codé en dur pour Linux/Unix

Trois correctifs, tous confirmés par un vrai échec de lien puis une vraie réussite après correction (jamais par supposition) :
- Pilote de lien : `cc` sur Unix, mais les distributions MinGW-w64/MSYS2 usuelles sur Windows ne fournissent QUE `gcc.exe`, pas systématiquement un alias `cc.exe` — `#[cfg(windows)]` choisit `"gcc"` au lieu de `"cc"`.
- `-no-pie` : concept ELF (relocations absolues vs PIE+TEXTREL, voir [securite-lien-no-pie](securite-lien-no-pie.md)) sans équivalent PE direct — retiré pour Windows (`#[cfg(not(windows))]`).
- Bibliothèques système manquantes : `ocara_runtime` utilise des sockets (`ocara.HTTPServer`/`ocara.HTTPRequest`) et des nombres aléatoires cryptographiques (`rand`, OpenSSL vendored) — sur Linux, ces symboles viennent de la libc ; sur Windows, ce sont des bibliothèques système séparées, jamais liées par défaut. Confirmé par échec de lien réel (`WSARecv`/`WSASend`/`WSAGetLastError`/`recv`/`send`/`closesocket`/`freeaddrinfo` non résolus en liant un simple "Hello World") : ajout de `-lws2_32 -lbcrypt -luserenv -lntdll` pour Windows.

### Vérification finale — bout en bout, sous Wine, pas seulement "ça compile"

- `ocara.exe` (PE32+ x86-64) démarre sous Wine et affiche correctement `--help`.
- Un vrai programme `.oc` ("Hello World") compilé PAR `ocara.exe` (sous Wine) produit un objet **COFF valide** (`--no-link`), qui une fois lié (avec les correctifs `link.rs` ci-dessus, testés manuellement avec le même `x86_64-w64-mingw32-gcc`) donne un `.exe` qui **s'exécute et affiche le bon texte sous Wine**.
- **Mutex + threads réels** : programme `.oc` avec 2 threads incrémentant un compteur partagé 1000 fois chacun sous un `ocara.Mutex` — résultat exact `2000` (aucune perte d'incrément, `CRITICAL_SECTION` fonctionne correctement sous charge concurrente réelle).
- **Sockets réels** : un `ocara.HTTPServer` compilé pour Windows, lancé sous Wine, répond à un vrai `curl` depuis l'hôte Linux (`HTTP 200`, corps correct) — confirme que `-lws2_32` et le reste de la pile réseau (`tiny_http`, `std::net` sur windows-gnu) fonctionnent réellement, pas juste "ça lie".
- `make build`/`make tests` (110 passed)/`make regression` (687 PASS, 0 FAIL) : inchangés côté hôte Linux après tous ces changements (fichiers partagés : `build.rs`, `runtime/src/mutex.rs`, `runtime/src/lib.rs`, `src/codegen/link.rs`).
- `make build-runtime-android` : toujours vert après ces mêmes changements partagés (le module `platform` de `mutex.rs` retombe sur la branche `#[cfg(unix)]`, Android étant une cible Unix).

## Ce qui reste

- **`ocara.exe` ne peut pas encore compiler un programme important `ocara.Tauri`** : WebView2 sur Windows exige l'ABI MSVC (son import lib officielle Microsoft est un `.lib` COFF/MSVC, illisible par `ar`/`ld` GNU) — probablement bloquant durablement avec MinGW seul. Piste identifiée mais non explorée : `cargo-xwin` (fournit les en-têtes/CRT MSVC sans avoir besoin de Windows ni de Visual Studio) ciblant `x86_64-pc-windows-msvc` au lieu de `-gnu` — un toolchain entièrement différent, pas une extension de celui-ci.
- **`ocara.SDL` n'a pas encore été essayé pour cette cible** — probablement plus simple que Tauri (SDL3/CMake a un support MinGW de longue date, et `x86_64-w64-mingw32-{gcc,ar}` suivent la convention de nommage que `cc`-rs/`cmake`-rs détectent automatiquement pour `windows-gnu`, sans fichier toolchain CMake dédié comme pour Android/NDK) — non vérifié empiriquement, à faire.
- **Jamais testé sur une vraie machine Windows** — seulement sous Wine 9.0 (64 bits ; le paquet `wine32:i386` n'a jamais été installé, hors périmètre puisque tout ce chantier vise exclusivement `x86_64-pc-windows-gnu`). Un utilisateur final sur un vrai Windows doit disposer d'un `gcc`/`cc` MinGW-w64 sur son `PATH` pour que `ocara.exe` puisse lier les programmes qu'il compile — exactement le même prérequis implicite que sur Linux (`ocara` y invoque aussi un `cc` externe au lien, jamais bundlé), pas une contrainte Windows spécifique.
- **Aucun script `.bat`/PowerShell dédié** — `ocara.exe` fonctionne tel quel en ligne de commande, mais rien n'aide un utilisateur Windows à l'installer/l'ajouter au `PATH` (équivalent des cibles `install`/`uninstall` du Makefile, inutilisables telles quelles sur Windows).
- **Pas de cible `make` pour tester automatiquement sous Wine** (l'équivalent Windows de `android-simulator`) — chaque vérification de ce ticket a été faite à la main.

## Priorité / Complexité

Le socle (compiler `ocara`/`ocara_runtime` et les faire tourner) s'est avéré une **addition contenue** une fois les bons outils en place (mingw-w64 + Wine) — comme pour Android, la plupart des blocages redoutés a priori (pthreads, build.rs non conscient de la cible, bibliothèques système manquantes) étaient de la vraie plomberie, précisément identifiable par lecture de code et confirmée par des échecs de compilation/lien réels, pas des inconnues. Tauri (MSVC/WebView2) est, comme documenté ci-dessus, une **Structurel voire Massif** à part entière (toolchain différent) si jamais entrepris.

## Fichiers clés

`runtime/src/mutex.rs` (module `platform`, `pub(crate)`, **fait**), `runtime/src/lib.rs` (`__alloc_locked_cell`/`__locked_cell_get`/`__locked_cell_set` réutilisent `crate::mutex::platform`, **fait**), `runtime/Cargo.toml` (`windows-sys` en dépendance `cfg(windows)` uniquement, **fait**), `build.rs` racine (`target_release_dir`, stub `.a` vide pour Tauri/SDL sur Windows, **fait**), `src/codegen/link.rs` (`link()` : pilote `gcc`/`cc`, `-no-pie` conditionnel, `-lws2_32`/`-lbcrypt`/`-luserenv`/`-lntdll` sur Windows, **fait**), `Makefile` (`build-runtime-windows`/`build-windows`, **fait**), [docs/windows.md](../windows.md) (procédure complète, **fait**), `runtime_tauri/` (WebView2/MSVC, **pas fait**, voir "Ce qui reste"), `runtime_sdl/` (**pas essayé**, voir "Ce qui reste").
