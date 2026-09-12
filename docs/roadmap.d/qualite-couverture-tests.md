# Couverture de tests et de CI incomplète

## ✅ Corrigé

- **Bug racine du test d'interfaces trouvé et corrigé** : `examples/tests/11_interfacesTest.oc` échouait à la compilation (`'Rectangle' is not a class`), pas seulement à cause des assertions commentées. Cause réelle : la déduplication des imports dans `src/main.rs` (section 4a) se faisait **par fichier seul** (`processed_files: HashSet<PathBuf>`) — dès qu'un fichier avait été chargé pour UN symbole (`import Circle from "11_interfaces"`), toute importation ultérieure d'un AUTRE symbole du même fichier (`import Rectangle from "11_interfaces"`) était silencieusement ignorée. C'est pourtant le cas d'usage canonique documenté par l'EBNF elle-même (§4.3, "Cas 2 : Import sélectif depuis fichier multi-classes"). **Corrigé** : déduplication par `(fichier, symbole demandé)` au lieu de fichier seul, avec mise en cache du programme parsé pour éviter de relire/reparser le fichier à chaque symbole.
- Les 4 assertions commentées de `examples/tests/11_interfacesTest.oc` sont réactivées (elles ne pouvaient de toute façon pas être testées avant la correction ci-dessus).
- `examples/from/import_from.oc` (seul point d'entrée exécutable de `examples/from/`) ajouté à `ci/regression.sh`.
- Nouveau test dédié à `consumed` dans `examples/tests/01_variablesTest.oc` (`consumedVariableTest`).

Vérifié : `make regression` passe de 313 à 318 PASS (0 FAIL) et de 5 à 4 ERREUR(S) — seule l'erreur `11_interfacesTest` a disparu, les 4 autres (pré-existantes, voir ci-dessous) sont inchangées.

## Toujours hors périmètre

- **`examples/project/tests/mainTest.oc` (`interface 'Printable' not found`)** : cause différente de celle corrigée ci-dessus — passe par `import main` (ancien format, un seul segment), qui résout au symbole **fonction** `main` du fichier via le second chemin de chargement redondant de `src/main.rs` (section 4b), lequel ne fusionne que classes/functions/consts, jamais interfaces/modules. C'est le bug déjà documenté dans [langage-imports-modules](langage-imports-modules.md) (Moyenne priorité / Structurel) — non traité ici, hors périmètre "Légère".
- **`examples/generics/`** : toujours absent de toute cible Makefile/CI, toujours cassé par la syntaxe `T[]` obsolète (voir [langage-syntaxe-obsolete](langage-syntaxe-obsolete.md), Basse priorité) — non traité ici.
- **`examples/advanced/{httpserver,mini_project,tauri_httpserver}/main.oc`** : évalués mais **volontairement pas ajoutés** à `ci/regression.sh`. Ces programmes démarrent un serveur HTTP qui bloque indéfiniment (`server.start()`/`server.run()`) — les ajouter à une boucle générique comme celle utilisée pour `examples/from/` bloquerait la CI. Les exemples déjà couverts (`examples/builtins/httpserver.oc`/`httpserver_static.oc`) contournent ça via un script dédié (`examples/builtins/httpserver.sh`, démarrage en fond + requête + arrêt) — il faudrait écrire l'équivalent pour chacun de ces 3 programmes (routes différentes, `tauri_httpserver` nécessite en plus un environnement graphique). Ampleur jugée au-delà de "Légère".
- **Infrastructure CI pour MySQL** : aucun service/container MySQL disponible dans cet environnement de développement pour valider un tel ajout — non traité (nécessite une vraie infrastructure CI, hors de ce qui peut être vérifié ici).

## Fichiers clés

`src/main.rs` (section 4a, boucle `imports_to_process`), `examples/tests/11_interfacesTest.oc`, `examples/tests/01_variablesTest.oc`, `ci/regression.sh`.
