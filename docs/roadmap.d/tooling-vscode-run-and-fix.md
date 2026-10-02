# VS Code : « Compiler et lancer » et « Fixer la mise en forme »

## Proposition

Deux nouvelles entrées du menu clic droit, dans l'éditeur et dans
l'arborescence, comme « Compiler le script » et « Afficher le dump » (voir
`tools/highlight/vsode/src/compile.ts`) :

- **Compiler et lancer** : compile le script (compilateur `ocara.compilerPath`)
  puis exécute le binaire **depuis le dossier du script**, dans un terminal
  intégré pour que l'entrée clavier (`IO::read`) et les programmes longs
  (serveur HTTP) fonctionnent.
- **Fixer la mise en forme** : lance `ocaracs --fix` (`ocara.ocaracsPath`) sur
  le fichier, ou sur le dossier depuis l'arborescence, puis relance l'analyse
  (`lint.ts`). Le document ouvert doit être enregistré avant et rechargé
  après.

## Points à trancher

- **Binaire de « Compiler et lancer »** : temporaire, supprimé à la fin comme
  pour le dump, ou nommé et conservé à côté du script (demande du nom comme
  « Compiler le script ») ?
- **Terminal** : un terminal « Ocara » réutilisé à chaque lancement, ou un
  nouveau à chaque fois ? Que faire si le programme précédent tourne encore
  (serveur) ?
- **Confirmation avant `--fix`** : `--fix` renomme dans tout le projet et peut
  renommer des fichiers. Faut-il une confirmation, ou au moins l'annonce des
  fichiers modifiés (sortie d'ocaracs dans le canal « Ocara ») ?
- **Fichiers renommés par `--fix`** : rouvrir l'onglet sous son nouveau nom.

## À mettre à jour

`tools/highlight/vsode/src/compile.ts` (ou un nouveau `run.ts`), `lint.ts`,
`package.json` (commandes, menus `editor/context` et `explorer/context`),
README de l'extension.

## Priorité / Complexité

Moyenne — **Légère**.
