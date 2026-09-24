# Ocara — Compilation croisée vers Windows

## État actuel

| Sous-chantier | État |
|----------------|------|
| Cross-compilation `ocara_runtime` vers `x86_64-pc-windows-gnu` | ✅ Fait, vérifié |
| Cross-compilation `ocara` (le compilateur lui-même) | ✅ Fait, vérifié — tourne réellement sous Wine |
| Lien final (`.exe`) et exécution réelle (mutex/threads, sockets/HTTP) | ✅ Fait, vérifié sous Wine |
| `ocara.Tauri` sur Windows | ❌ Pas fait — WebView2 exige l'ABI MSVC, incompatible avec MinGW (voir [roadmap.d/packaging-windows.md](roadmap.d/packaging-windows.md)) |
| `ocara.SDL` sur Windows | ❌ Pas encore essayé |
| Test sur une vraie machine Windows | ❌ Seulement sous Wine 9.0 (64 bits) |

Historique complet, pièges rencontrés et vérifications détaillées : [roadmap.d/packaging-windows.md](roadmap.d/packaging-windows.md).

64 bits **uniquement** (`x86_64-pc-windows-gnu`) — 32 bits (`i686-*`) explicitement hors périmètre, jamais visé par ce projet.

---

## Prérequis

### Installer le toolchain MinGW-w64 (cross-compilation depuis Linux)

```bash
sudo apt-get install gcc-mingw-w64-x86-64
```

PAS le paquet `mingw-w64` générique (qui installe aussi le 32 bits `i686-*`, jamais utilisé ici) — uniquement `gcc-mingw-w64-x86-64`, qui fournit `x86_64-w64-mingw32-gcc`/`-ar`/`-ld`/`-ranlib`.

### Ajouter la cible Rust

```bash
rustup target add x86_64-pc-windows-gnu
```

### Installer Wine (pour vérifier un binaire produit, sans machine Windows)

```bash
sudo apt-get install wine
```

`wine --version` peut afficher un avertissement sur `wine32:i386` manquant au premier lancement — sans rapport avec ce chantier (ce message concerne l'exécution de binaires **32 bits**, jamais produits ici) : ignorable.

---

## 1. Compiler le runtime Ocara pour Windows (une seule fois, ou après modification de `runtime/`)

```bash
make build-runtime-windows
```

Cross-compile `ocara_runtime` (SQLite bundled, OpenSSL vendored, zlib static, tout le reste des dépendances C) via `x86_64-w64-mingw32-gcc` comme compilateur C croisé. Produit `target/x86_64-pc-windows-gnu/release/libocara_runtime.a`.

**Vérifié réellement** : compile avec `RUSTFLAGS="-D warnings"`, 0 warning, sur l'ensemble des dépendances (rusqlite, mysql, openssl vendored, libz-sys static, ureq, tiny_http, regex, serde...) — le seul point qui a réellement bloqué à la première tentative est documenté au §"Pièges rencontrés" ci-dessous.

## 2. Compiler `ocara` lui-même pour Windows

```bash
make build-windows
```

Dépend de `build-runtime-windows` (voir ci-dessus — le propre `build.rs` de `ocara` embarque `libocara_runtime.a` via `include_bytes!`). Produit `target/x86_64-pc-windows-gnu/release/ocara.exe` — un vrai exécutable Windows (PE32+ x86-64), pas un binaire Linux.

`ocara_runtime_tauri`/`ocara_runtime_sdl` ne sont pas encore disponibles pour cette cible (voir [roadmap.d/packaging-windows.md](roadmap.d/packaging-windows.md)) — un programme qui importerait `ocara.Tauri`/`ocara.SDL`, compilé par CET `ocara.exe`, échouerait au lien final avec des symboles non résolus (échec explicite, pas un binaire cassé silencieusement).

## 3. Vérifier `ocara.exe` sous Wine

```bash
wine target/x86_64-pc-windows-gnu/release/ocara.exe --help
```

**Vérifié réellement** : affiche l'aide normalement. Pour compiler un vrai programme :

```bash
wine target/x86_64-pc-windows-gnu/release/ocara.exe mon_programme.oc --no-link -o mon_programme
```

Produit un objet **COFF** valide (`file mon_programme.o` → `Intel amd64 COFF object file`). Sans `--no-link`, `ocara.exe` tente d'invoquer `gcc` pour le lien final — **sous Wine, ce `gcc` doit lui-même être un exécutable Windows natif** (Wine ne peut pas appeler le `x86_64-w64-mingw32-gcc` du côté Linux depuis un processus qu'il émule) : sans un MinGW-w64 natif installé dans le préfixe Wine, lier directement via `ocara.exe --no-link` absent ne fonctionne pas sous Wine — c'est attendu, voir §4 pour la façon dont ce chantier a vérifié le lien complet malgré tout.

Sur une **vraie machine Windows**, ce problème ne se pose pas : `ocara.exe` invoque `gcc` normalement, à condition qu'un MinGW-w64 natif soit installé et sur le `PATH` (ex: via [MSYS2](https://www.msys2.org/)) — exactement le même prérequis implicite qu'un `cc` sur Linux, jamais bundlé par `ocara` lui-même.

## 4. Vérifier le lien complet (compilation + lien + exécution réelle)

Puisque Wine ne peut pas exécuter le `gcc` croisé du côté Linux, ce chantier a vérifié le lien final en le reproduisant manuellement avec le MÊME toolchain que celui qu'utiliserait `ocara.exe` sur une vraie machine Windows :

```bash
# 1. Compiler l'objet (sous Wine, via ocara.exe)
wine target/x86_64-pc-windows-gnu/release/ocara.exe mon_programme.oc --no-link -o mon_programme

# 2. Lier manuellement (mêmes flags que src/codegen/link.rs, côté Windows)
x86_64-w64-mingw32-gcc mon_programme.o \
  target/x86_64-pc-windows-gnu/release/libocara_runtime.a \
  -o mon_programme.exe -lm -Wl,--allow-multiple-definition -Wl,--gc-sections -Wl,--as-needed \
  -lws2_32 -lbcrypt -luserenv -lntdll

# 3. Exécuter sous Wine
wine mon_programme.exe
```

**Vérifié réellement**, pas juste "ça devrait marcher" :
- Un `"Hello World"` compilé ainsi affiche le bon texte sous Wine.
- Un programme avec 2 `Thread`/`Mutex` incrémentant un compteur partagé 1000 fois chacun donne exactement `2000` (aucune perte d'incrément — `CRITICAL_SECTION`, l'équivalent Windows de `pthread_mutex`, fonctionne correctement sous charge concurrente réelle).
- Un `ocara.HTTPServer` lancé sous Wine répond à un vrai `curl` depuis l'hôte Linux (`HTTP 200`, corps correct) — confirme que les sockets (`-lws2_32`) et toute la pile réseau fonctionnent réellement.

---

## Pièges rencontrés (résolus)

- **`pthread_mutex_*` n'existe pas dans `libc` pour Windows** (`runtime/src/mutex.rs`, et un deuxième site interne dans `runtime/src/lib.rs`) — Windows n'a pas de pthreads natif ; corrigé avec `CRITICAL_SECTION` (`windows-sys`) derrière un module `platform` conditionnel par plateforme. Voir [roadmap.d/packaging-windows.md](roadmap.d/packaging-windows.md) pour le détail complet.
- **`build.rs` (racine) n'était pas conscient de la cible lors d'une cross-compilation** — embarquait silencieusement le runtime de l'HÔTE (Linux) au lieu de celui cross-compilé pour Windows. Corrigé (`target_release_dir()`, comparaison `TARGET`/`HOST`).
- **`src/codegen/link.rs` codé en dur pour Linux** — pilote de lien (`cc` n'existe pas forcément sous Windows, `gcc` oui), `-no-pie` (concept ELF sans équivalent PE), bibliothèques système manquantes pour les sockets/la génération de nombres aléatoires (`-lws2_32`/`-lbcrypt`/`-luserenv`/`-lntdll`, jamais nécessaires sur Linux où ces symboles viennent de la libc).

## Limitations connues

- **`ocara.Tauri` non disponible sur Windows** — WebView2 exige l'ABI MSVC (import lib `.lib` COFF/MSVC, illisible par `ar`/`ld` GNU) ; piste identifiée mais non explorée : `cargo-xwin` + cible `x86_64-pc-windows-msvc`, un toolchain entièrement différent de celui utilisé ici.
- **`ocara.SDL` non essayé sur Windows** — probablement plus simple que Tauri (support MinGW de longue date côté SDL3/CMake), non vérifié.
- **Jamais testé sur une vraie machine Windows** — uniquement sous Wine 9.0. Un utilisateur final doit avoir un MinGW-w64 natif (`gcc`) sur son `PATH` pour que `ocara.exe` puisse lier les programmes qu'il compile.
- **Aucun script d'installation Windows** (`.bat`/PowerShell) — `ocara.exe` fonctionne tel quel, mais rien n'automatise son ajout au `PATH`.
