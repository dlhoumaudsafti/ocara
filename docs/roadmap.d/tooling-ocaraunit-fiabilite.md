# ocaraunit : cache sans limite (~110 Go), blocages, cache périmé, tests ignorés

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Disque et blocages

1. **Le cache grossit sans fin.** Mesuré : `examples/.ocaraunit_cache` 95 Go,
   `examples/advanced/mini_project_hexa/.ocaraunit_cache` 9,8 Go (1 270
   fichiers, 96 versions de `FormatTest`), `examples/project/.ocaraunit_cache`
   5,2 Go, `./.ocaraunit_cache` 663 Mo. Chaque binaire est `<stem>-<hash>`,
   son `.o` est gardé, rien n'est purgé (`tools/ocaraunit/src/main.rs` ~l.427,
   commentaire ~l.1036). Garder la dernière entrée par fichier (ou un plafond
   LRU) et supprimer le `.o`. Le cache est en plus créé dans le dossier
   courant, pas à la racine du projet.
2. **Un test qui boucle bloque le runner indéfiniment** (`Command::output()`
   sans délai, ~l.479). Ajouter `--timeout` qui tue le binaire.

## Résultats faux (tests qui passent à tort)

3. **Cache périmé si une classe importée est hors de la racine** :
   `import Helper from "../lib/Helper"`, modification de `lib/Helper.oc` →
   « PASS (cached) » alors que le test devrait échouer (l'empreinte ne couvre
   que les `.oc` sous `project_root`, ~l.338-347).
4. **Découverte fragile (tests ignorés, exit 0)** :
   - fonctions libres de test dans un fichier qui déclare aussi une classe
     (~l.204-222) ;
   - `public  method xTest` (deux espaces) ou tabulation (`strip_prefix
     ("public method ")`, ~l.210) ;
   - fichier qui déclare déjà `function main()` : le `main` généré est en
     double, le compilateur garde le premier (voir
     [sema-solidite-typage.md](sema-solidite-typage.md) n°13) ;
   - fichier `*Test.oc` sans test compté comme succès.
   Utiliser le parseur du compilateur pour découvrir les tests.
5. **Exit 0 sans aucun test** : `--coverage` consomme le dossier de tests
   comme dossier source (~l.660-749) ; dossier `tests/` absent ou vide ;
   options inconnues et second argument ignorés.

## Ergonomie

- Une assertion en échec est comptée en ERREUR, jamais en FAIL (la colonne
  FAIL vaut toujours 0 ; l'exemple du README est faux) ; le premier échec
  arrête tout le fichier.
- Binaire qui plante : « ERREUR exécution : » sans signal ni code
  (`ExitStatusExt::signal`, ~l.509).
- Racine mal détectée hors d'un dossier `tests/` (« Not a directory ») ;
  `OCARA` relatif résolu depuis le dossier de l'enfant (~l.360) ; imports
  relatifs vers l'extérieur de la racine non réécrits (~l.261-272).
- Erreurs de compilation qui citent `__ocaraunit__X.oc` (supprimé) avec des
  lignes du `main` généré ; codes ANSI bruts quand la sortie est redirigée.
- Exclusions `.ocaraunit` par sous-chaîne (`src/` exclut `mysrc/`) ; pas de
  parallélisme ; une sortie « PASS … » imprimée par le test compte comme une
  assertion.

## Priorité / Complexité

**Haute** (disque saturé, tests faux positifs) — **Légère**.

## Fichiers clés

`tools/ocaraunit/src/main.rs`, `tools/ocaraunit/README.md`.
