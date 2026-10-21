# Tests des blocs runtime : `RuntimeTest.oc`, builtins bloquants neutralisés

Demande du 2026-10-21, suite à la couverture par ligne (`ocaraunit
--line-coverage`) : les fichiers `*.runtime.oc` d'un projet (par exemple
`examples/advanced/mini_project_hexa/core/`) ne sont jamais exécutés par les
tests, donc jamais couverts.

## Constat

- Un bloc runtime (`runtime core.init is init`) n'est ni une classe ni une
  fonction : ses instructions sont injectées dans le `main` synthétisé du
  point d'entrée (`init → main → success|error → exit`). Un test ocaraunit a
  son propre `main` généré : il ne peut ni importer un bloc runtime, ni
  exécuter le `main` de l'application.
- Le lancement réel bloque ou exige un écran : `use Tauri(...).run()` (fenêtre),
  boucle SDL, `HTTPServer.run`/`start` (jamais de retour), et une variante
  `when System::OS is 'android'` ne s'exécute pas sur l'hôte.

## Évolution retenue

1. **`RuntimeTest.oc`** : un fichier de test représente la **globalité de la
   fonction `main`** générée par les blocs runtime du point d'entrée, tous
   les blocs rassemblés. ocaraunit compile et exécute ce `main` complet
   (instrumenté par `--coverage`), puis les tests du fichier, qui vérifient
   le résultat du flot et ses effets observables (requêtes HTTP, base,
   fichiers produits). Convention de nommage : `tests/RuntimeTest.oc` pour le
   `main.oc` du projet.
2. **Nouvelles assertions `UnitTest`**, sur le résultat du `main` exécuté
   (aucun argument) :
   - `assertSuccessResult()` — le flot s'est terminé par `result SUCCESS` ;
   - `assertErrorResult()` — il s'est terminé par `result ERROR` ;
   - `assertErrorResultCode(code:int)` — `result ERROR` avec ce code.
3. **Déclarations `when` prises en compte** : un `RuntimeTest.oc` est compilé
   et exécuté une fois par variante `when` des déclarations runtime du point
   d'entrée (ex. `core.mainAndroid` pour `System::OS is 'android'`), la
   configuration étant forcée sur un binaire hôte (nouvelle option du
   compilateur : configuration `when` imposée à la génération de code — le
   compilateur analyse déjà toutes les variantes).
4. **Builtins bloquants étiquetés et neutralisés en test** : chaque builtin
   qui bloque porte une étiquette dans sa description — **thread principal**
   (`Tauri.run`, boucle SDL) ou **bloquant** (`HTTPServer.run`/`start`,
   `Thread.join`, `Thread::sleep`, lectures `IO`…). Dans un binaire de test
   ocaraunit, tout builtin étiqueté bloquant évalue ses arguments, enregistre
   l'appel et rend la main : la ligne est exécutée (donc couverte) et le flot
   va jusqu'au bout.
5. **Assertions de blocage, résolues à la compilation** :
   - `assertBlocksMainThread(expr)` — `expr` occupe le thread principal
     (boucle d'événements : Tauri, SDL) ;
   - `assertIsBlocking(expr)` — `expr` bloque le thread appelant.

   `expr` est analysée, **jamais évaluée** (savoir si un appel bloque en
   l'exécutant est indécidable en général, et ouvrirait réellement la
   fenêtre) : étiquette du builtin appelé, propagée aux méthodes utilisateur
   par le graphe d'appels. Exemple :
   ```ocara
   self::assertBlocksMainThread(use Tauri({"title": "Garage Ocara", "url": "http://localhost:8081"}).run())
   ```

## Lots

1. `RuntimeTest.oc` (ocaraunit : `main` complet du point d'entrée au lieu du
   `main` de test) + `assertSuccessResult`/`assertErrorResult`/
   `assertErrorResultCode`.
2. Étiquettes « bloquant » / « thread principal » des builtins et
   neutralisation dans les binaires de test.
3. Variantes `when` : option du compilateur forçant la configuration,
   ocaraunit exécute `RuntimeTest.oc` par variante.
4. `assertBlocksMainThread` / `assertIsBlocking` (analyse statique,
   propagation par le graphe d'appels).

Chaque lot : `docs/builtins/UnitTest.md`, `tools/ocaraunit/README.md`,
`docs/adding-builtins.md` (étiquette bloquante), extension VS Code
(couverture des fichiers runtime), test de régression sur
`examples/advanced/mini_project_hexa` (`tests/RuntimeTest.oc`).

## Priorité / Complexité

**Moyenne** — **Structurel** (compilateur, runtime, UnitTest, ocaraunit).

## Fichiers clés

`tools/ocaraunit/src/runner.rs`, `tools/ocaraunit/src/discovery.rs`,
`src/core/runtime_expand.rs`, `src/lower/builder.d/runtime.rs`,
`src/builtins/*.rs` (descriptions), `runtime/src/lib.rs` (UnitTest),
`src/parsing/build.rs` (configuration `when`), `docs/builtins/UnitTest.md`.
