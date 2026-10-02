# VS Code : « Compiler et lancer » et « Fixer la mise en forme » — implémenté

Documentation utilisateur : `tools/highlight/vsode/README.md`.

## Ce qui a été tranché

- **Compiler et lancer** (`ocara.compileAndRun`, `src/compile.ts`) : le nom du
  binaire est demandé, comme pour « Compiler le script » ; il est créé et
  conservé à côté du script. Le binaire est exécuté depuis le dossier du
  script dans un terminal intégré « Ocara » **unique**, partagé par tous les
  lancements. Le programme précédent y est arrêté (Ctrl+C) avant la relance.
- **Fixer la mise en forme** (`ocara.fixStyle`, `src/fix.ts`) : sur un script
  (éditeur, arborescence) ou un dossier (arborescence). Les scripts modifiés
  sont d'abord enregistrés, puis `ocaracs --fix --dry-run` est lancé. La
  confirmation, qui liste les renommages, n'est demandée **que si** des
  identifiants doivent être renommés. Ensuite :
  - les onglets des fichiers renommés sont rouverts sous leur nouveau nom ;
  - l'analyse est relancée (`OcaracsLinter.relint`) ;
  - le compte rendu va dans le canal « Ocara », désormais partagé
    (`ocaraOutput`, `src/toolrunner.ts`).
- **ocaracs** : nouvelle option `--fix --dry-run`, qui affiche le plan sans
  rien écrire. Corrigé au passage : `ocaracs --fix x.oc` (sans dossier)
  échouait à lire le dossier racine, car le parent du chemin était vide.

## Non vérifié

Le comportement dans VS Code lui-même (menus, terminal, boîte de
confirmation) n'a pas été testé automatiquement : l'extension compile, est
réinstallée, et ocaracs `--dry-run`/`--fix` est vérifié en ligne de commande.

## Bug trouvé

Arguments de `use Classe(...)` ignorés pour une classe sans `init` → voir
[sema-use-args-without-init](sema-use-args-without-init.md).
