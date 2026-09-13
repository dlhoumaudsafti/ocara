# Couverture de tests et de CI incomplète

## ✅ Corrigé

- **Bug racine du test d'interfaces trouvé et corrigé** : `examples/tests/11_interfacesTest.oc` échouait à la compilation (`'Rectangle' is not a class`), pas seulement à cause des assertions commentées. Cause réelle : la déduplication des imports dans `src/main.rs` (section 4a) se faisait **par fichier seul** (`processed_files: HashSet<PathBuf>`) — dès qu'un fichier avait été chargé pour UN symbole (`import Circle from "11_interfaces"`), toute importation ultérieure d'un AUTRE symbole du même fichier (`import Rectangle from "11_interfaces"`) était silencieusement ignorée. C'est pourtant le cas d'usage canonique documenté par l'EBNF elle-même (§4.3, "Cas 2 : Import sélectif depuis fichier multi-classes"). **Corrigé** : déduplication par `(fichier, symbole demandé)` au lieu de fichier seul, avec mise en cache du programme parsé pour éviter de relire/reparser le fichier à chaque symbole.
- Les 4 assertions commentées de `examples/tests/11_interfacesTest.oc` sont réactivées (elles ne pouvaient de toute façon pas être testées avant la correction ci-dessus).
- `examples/from/import_from.oc` (seul point d'entrée exécutable de `examples/from/`) ajouté à `ci/regression.sh`.
- Nouveau test dédié à `consumed` dans `examples/tests/01_variablesTest.oc` (`consumedVariableTest`).
- **`examples/advanced/httpserver/main.oc` ajouté à la CI** : nouveau script `examples/advanced/httpserver/httpserver.sh` (même mécanisme que `examples/builtins/httpserver.sh` — démarrage en fond, requêtes sur les 6 routes + cas 404, arrêt), câblé dans `ci/regression.sh`. Sondage de démarrage sur `/about` plutôt que `/` : `/` charge deux statistiques en parallèle via `async`/`resolve` (`Thread::sleep` 3s + 5s) et répond donc volontairement ~5s après le démarrage réel du serveur.
- **`builtins/mysql` ne fait plus échouer la suite en local** : `ci/regression.sh` sonde maintenant `127.0.0.1:3306` (configurable via `MYSQL_HOST`/`MYSQL_PORT`) avant de lancer ce test précis, et l'annonce comme `SKIP` (pas un échec) si aucun serveur n'est joignable — au lieu de `FAIL` à chaque exécution locale faute d'infrastructure.

Vérifié : `make regression` passe de 313 à 318 PASS (0 FAIL) côté ocaraunit, et `ci/regression.sh` seul se termine maintenant en succès (« Tous les tests ont réussi » — `builtins/mysql` skippé au lieu d'échouer, `advanced_httpserver` passe). Le `make regression` global reste en échec uniquement à cause des erreurs de compilation déjà documentées ci-dessous (sans rapport avec ce qui a été corrigé ici).

## Corrigé mais nuance importante découverte en creusant

**`mini_project` ET `tauri_httpserver` ouvrent tous les deux une vraie fenêtre Tauri (WebView)** — pas seulement `tauri_httpserver` comme documenté précédemment ici. `mini_project/main.oc` importe aussi `ocara.Tauri` et appelle `ui.run()` en plus de son serveur HTTP/SQLite. Décision prise avec David : ces deux exemples restent **volontairement hors CI** (pas de tentative de test headless via Xvfb) — seul `examples/advanced/httpserver/` (serveur HTTP pur, sans GUI) est couvert.

## Toujours hors périmètre

- **`examples/project/tests/mainTest.oc` (`interface 'Printable' not found`)** : cause différente de celle corrigée ci-dessus — passe par `import main` (ancien format, un seul segment), qui résout au symbole **fonction** `main` du fichier via le second chemin de chargement redondant de `src/main.rs` (section 4b), lequel ne fusionne que classes/functions/consts, jamais interfaces/modules. C'est le bug déjà documenté dans [langage-imports-modules](langage-imports-modules.md) (Moyenne priorité / Structurel) — non traité ici, hors périmètre "Légère".
- **`examples/generics/`** : toujours absent de toute cible Makefile/CI, toujours cassé par la syntaxe `T[]` obsolète (voir [langage-syntaxe-obsolete](langage-syntaxe-obsolete.md), Basse priorité) — non traité ici.
- **`mini_project`/`tauri_httpserver`** : hors CI par décision explicite (voir ci-dessus), pas par manque de temps.
- **Infrastructure CI réelle pour MySQL** : le skip local ne fait qu'éviter un faux échec ; aucun serveur n'est réellement testé ici (pas de Docker disponible dans cet environnement de dev, pas de `mariadb-server` installé). Pour qu'un vrai test MySQL tourne un jour en CI (ex. GitHub Actions), fournir un service MySQL au runner :

  ```yaml
  jobs:
    regression:
      runs-on: ubuntu-latest
      services:
        mysql:
          image: mysql:8
          env:
            MYSQL_ALLOW_EMPTY_PASSWORD: yes
            MYSQL_DATABASE: test_db
          ports:
            - 3306:3306
          options: >-
            --health-cmd="mysqladmin ping"
            --health-interval=5s --health-timeout=3s --health-retries=5
      steps:
        - uses: actions/checkout@v4
        - run: make build
        - run: make regression   # builtins/mysql s'exécute réellement : le port 3306 répond
  ```

  Ce projet n'a aujourd'hui aucun fichier de pipeline CI versionné (`.github/workflows/`, `.gitlab-ci.yml`, ...) — ce snippet est un point de départ pour le jour où il en aura un, pas une configuration active.

## Fichiers clés

`src/main.rs` (section 4a, boucle `imports_to_process`), `examples/tests/11_interfacesTest.oc`, `examples/tests/01_variablesTest.oc`, `ci/regression.sh`, `examples/advanced/httpserver/httpserver.sh`.
