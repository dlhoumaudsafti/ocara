# Extension VS Code : analyse ocaracs automatique, compilation et dump depuis l'éditeur

## Demande (David)

### 1. Analyse ocaracs automatique

- À l'ouverture/l'affichage d'un fichier `*.oc` ou `*.ocara`, lancer
  `ocaracs` sur le fichier et publier ses avertissements comme
  diagnostics VS Code.
- Chaque ligne concernée est **surlignée en jaune** ; le message s'affiche
  **au survol** de la ligne (et dans le panneau Problèmes).
- **Configuration utilisateur** : chemin de `ocaracs` ou commande pour le
  lancer (ex. `ocaracs`, `/usr/local/bin/ocaracs`,
  `${workspaceFolder}/target/release/ocaracs`).
- **Règles** : fichier `.ocaracs` à la racine du projet, sinon dans le
  dossier du script, sinon paramètres par défaut d'ocaracs.

### 2. Compilation depuis l'éditeur

- Entrée de **menu contextuel** (clic droit) dans l'éditeur ET sur un
  fichier `.oc`/`.ocara` de l'arborescence : « Compiler le script ».
- Un **prompt** demande le nom du binaire à produire (valeur proposée : le
  nom du script sans extension).
- **Configuration utilisateur** : chemin de `ocara` ou commande pour le
  lancer (ex. `ocara`, `/usr/local/bin/ocara`,
  `${workspaceFolder}/target/release/ocara`).
- Sortie/erreurs du compilateur dans un canal de sortie, erreurs aussi
  publiées comme diagnostics.

### 3. Dump depuis l'éditeur

- Même entrée de menu contextuel (éditeur + arborescence) : « Afficher le
  dump ».
- Le dump s'ouvre dans un **éditeur sans fichier** (document virtuel /
  `untitled`), jamais enregistré sur disque.

## Constats (vérifiés dans le code)

- **ocaracs** : sortie déjà au format `fichier:ligne:col: warning: message`
  (`tools/ocaracs/src/main.rs`, `emit`) — directement parsable en
  diagnostics. Couleurs ANSI seulement si stderr est un terminal (pas le
  cas lancé depuis l'extension). Code de sortie `1` = avertissements
  trouvés (pas une erreur d'exécution).
- **Recherche du `.ocaracs`** (`find_project_root`) : ocaracs remonte DÉJÀ
  depuis le dossier du script jusqu'à la racine, et prend le PREMIER
  `.ocaracs` trouvé (donc le plus proche du script), sinon ses valeurs par
  défaut. L'ordre demandé (« racine du projet, puis du script ») est
  l'inverse — à trancher : garder la règle actuelle d'ocaracs (le plus
  proche gagne, convention habituelle type `.editorconfig`/`.eslintrc`),
  ou passer explicitement le fichier de config (option ocaracs à ajouter,
  ex. `--config <chemin>`) depuis l'extension.
- **ocaracs analyse aussi les imports** (`check_with_imports`) quand on lui
  passe un fichier : pour un affichage par fichier, ne garder que les
  diagnostics du fichier affiché (filtrer sur le chemin), ou ajouter une
  option `--no-imports`.
- **`ocara --dump`** (`src/core/cli.rs`) affiche tokens/AST et l'IR
  (`=== HIR ...`) sur stdout, mais **continue ensuite la compilation**
  jusqu'au binaire (`src/main.rs`) : pour « afficher sans enregistrer »,
  soit compiler vers un fichier temporaire supprimé aussitôt, soit (plus
  propre) faire s'arrêter `--dump` après l'affichage, ou combiner avec
  `--check`/`--no-link` — à vérifier/trancher.
- **Extension `.ocara`** : le langage ne déclare aujourd'hui que `.oc`
  (`tools/highlight/vsode/package.json`, `contributes.languages`) — à
  ajouter pour que coloration, complétion, CodeLens et la nouvelle analyse
  s'appliquent aussi à `*.ocara` (et vérifier que `ocara`/`ocaracs`
  acceptent cette extension, ex. résolution des imports et découverte de
  fichiers par ocaracs qui filtre sur `.oc`).

## Décisions et mise en œuvre

- **`.ocaracs`** : règle actuelle d'ocaracs conservée — le plus proche en
  remontant depuis le dossier du script, sinon ses valeurs par défaut.
  Aucun changement d'ocaracs.
- **Dump** : `--dump` inchangé ; l'extension compile vers un dossier
  temporaire supprimé aussitôt (binaire ET `.o`), seul le texte est ouvert
  dans un document sans fichier.
- **`.ocara`** : abandonné — `.oc` uniquement.

Fichiers (`tools/highlight/vsode/src/`) : `toolrunner.ts` (commande
configurable, découpe avec guillemets, variables `${workspaceFolder}`/
`${fileDirname}`, repli sur `<workspace>/target/release/<outil>` pour un
réglage laissé par défaut et absent du PATH ; parsing
`fichier:ligne:col: warning|error: message`), `lint.ts` (diagnostics
avertissement sur la ligne entière + décoration de fond jaune, à
l'ouverture/l'affichage/l'enregistrement ; avertissements des fichiers
importés filtrés), `compile.ts` (commandes `ocara.compile`/`ocara.dump`,
prompt du nom de binaire, erreurs → panneau Problèmes + sortie « Ocara »).
`package.json` : réglages `ocara.compilerPath`/`ocara.ocaracsPath`/
`ocara.lint.enable`, menus `editor/context`/`explorer/context`/palette.

Limite : ocaracs lit le fichier sur disque — analyse de la dernière version
enregistrée (relancée à chaque enregistrement), pas des modifications en
cours de frappe.

## Priorité / Complexité

**Haute** (demandé explicitement) — **Légère** côté extension ; plus
**Légère** côté outils si l'on ajoute `--config`/`--no-imports` à ocaracs
ou un `--dump` qui s'arrête après l'affichage.

## Fichiers clés

`tools/highlight/vsode/package.json`, `tools/highlight/vsode/src/extension.ts`
(+ nouveaux modules lint/compile), `tools/highlight/vsode/README.md`,
`tools/ocaracs/src/main.rs`, `src/core/cli.rs`, `src/main.rs`.
