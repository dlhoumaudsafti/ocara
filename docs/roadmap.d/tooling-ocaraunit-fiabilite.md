# ocaraunit : cache sans limite (~110 Go), blocages, cache périmé, tests ignorés

Audit du 2026-10-19 ; corrigé le 2026-10-20 (sauf « Reste »). Le runner est
découpé en modules (`tools/ocaraunit/src/`) : `cli.rs`, `config.rs`,
`scan.rs`, `discovery.rs`, `imports.rs`, `cache.rs`, `process.rs`,
`runner.rs`, `coverage.rs`, `report.rs` ; tests unitaires dans `tests.rs`.

## Corrigé

1. **Cache sans fin** (`cache.rs`) : une entrée par fichier de test,
   `<stem>-<hash chemin>-<hash contenu>` ; après chaque compilation,
   `purge` supprime les autres versions du même test, les entrées de l'ancien
   format `<stem>-<hash>` du même stem et tous leurs `.o` ; le `.o` du binaire
   courant est supprimé dès la compilation (`runner.rs::compile`). Le cache
   est créé à la racine du projet (`main.rs`), plus dans le dossier courant.
   Les anciens caches géants sont purgés au fil des recompilations.
2. **Test qui boucle** (`process.rs::run`) : `--timeout <s>` (défaut 120 s,
   le plus long fichier des exemples tourne ~45 s) appliqué à la compilation
   et à l'exécution ; au-delà le processus est tué et compté en ERREUR.
3. **Cache périmé** (`imports.rs::import_closure`, `cache.rs::test_key`) :
   la clé d'un test couvre la fermeture transitive de ses imports, résolus
   comme le compilateur (`./`, `../`, `a.b.X`, namespace), y compris hors de
   la racine — en plus de l'empreinte compilateur + `.oc` sous la racine.
4. **Découverte** (`scan.rs`, `discovery.rs::declarations`) : analyse
   lexicale (commentaires et chaînes retirés, profondeur d'accolades) ;
   fonctions libres de test détectées même à côté d'une classe ; espaces
   multiples et tabulations tolérés ; méthodes de `module`/`interface`/
   `generic` jamais appelées ; fichier qui déclare `function main()` → ERREUR
   explicite ; `*Test.oc` sans test → ERREUR.
5. **Exit 0 sans test** (`cli.rs::parse_args`, `main.rs::select_tests`) :
   `--coverage` n'avale plus le dossier de tests (`--coverage=<chemin>`, ou
   forme à deux positionnels `--coverage src/A.oc tests/ATest.oc` de la CI) ;
   dossier absent ou sans `*Test.oc` → exit 1 ; option inconnue, argument en
   trop, valeur invalide → exit 2.

Ergonomie :

- Chaque test est appelé dans un `try … on e is UnitTestException` généré
  (`discovery.rs::generate_main`) : une assertion en échec est un **FAIL**
  (`FAIL [Classe.test] message`) et n'arrête plus que son test ; une
  `UnitTestException` non rattrapée (constructeur) est aussi classée FAIL
  (`runner.rs::uncaught_assertion`). Exemple du README corrigé.
- Fin anormale : `tué par le signal 11 (SIGSEGV)` / `code de sortie 3`
  (`process.rs::describe_status`).
- Racine : parent du dossier `tests` le plus proche, sinon dossier du
  `.ocaraunit`, sinon dossier du test (`cli.rs::detect_project_root`) ;
  `OCARA` et chemins relatifs du binaire résolus depuis le dossier de
  lancement ; imports relatifs sortant de la racine réécrits en `../…`
  (`imports.rs::rebase_imports`).
- Diagnostics de compilation ramenés au fichier de test d'origine (le
  `main` généré est en fin de fichier, les numéros de ligne sont exacts) ;
  codes ANSI retirés quand la sortie n'est pas un terminal.
- Exclusions `.ocaraunit` par composants de chemin (`config.rs::is_excluded`).

## Reste

- Une ligne « PASS … » imprimée par le code testé compte comme une assertion
  (le runtime écrit `PASS`/`FAIL` sur stdout sans marqueur dédié).
- Découverte par analyse lexicale, pas par le parseur du compilateur
  (crate séparé) ; une chaîne gabarit imbriquée dans `${…}` d'une autre
  chaîne gabarit n'est pas gérée.
- Pas de parallélisme entre fichiers de test.

Règle de couverture inchangée (exigence de 100 % de la CI) : dans un fichier
à classe, seules les méthodes comptent, sinon les fonctions libres ; `main`,
`init` et les déclarations `async` sont exclus (`coverage.rs`). Les tests
Rust d'ocaraunit tournent avec `make tests-tools`.

## Priorité / Complexité

**Basse** (reste) — **Légère**.

## Fichiers clés

`tools/ocaraunit/src/`, `tools/ocaraunit/README.md`.
