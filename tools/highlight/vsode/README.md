# Ocara Language — Extension VS Code

Coloration syntaxique, et via le serveur de langage du compilateur : diagnostics en direct, autocomplétion, survol, navigation, références et CodeLens pour le langage **Ocara** (`.oc`).

## Fonctionnalités

- **Coloration syntaxique** complète (grammaire TextMate `syntaxes/ocara.tmLanguage.json`) : mots-clés,
  types, classes, structs, méthodes, imports, chaînes, templates, noms d'arguments nommés. Dans un
  template, `` \` `` et `\${x}` sont des échappements ; le HTML y est coloré (balises, attributs,
  commentaires, entités) sans jamais déborder au-delà du `` ` `` fermant.
- **Serveur de langage du compilateur** (`ocara --lsp`, lancé avec le compilateur du réglage
  `ocara.compilerPath` ; changer ce réglage redémarre le serveur, et la commande **« Ocara : Redémarrer
  le serveur de langage »** le relance à la demande, par exemple après avoir recompilé `ocara`) — toute
  la sémantique vient du compilateur lui-même, aucune heuristique :
  - **erreurs et avertissements pendant la frappe**, y compris ceux d'un fichier importé (signalés en
    tête du document, avec un lien) ; une erreur de syntaxe n'empêche pas le reste du document de
    fonctionner (toutes les erreurs de syntaxe sont signalées, survol et complétion continuent) ;
  - **survol** : type réel d'une variable, signature et commentaires `//` d'une déclaration,
    documentation des méthodes builtin (extraite de `docs/builtins/*.md`), méthode réellement appelée
    par le sucre d'instance (`s.trim()`, `s.toInt()`), mots-clés (résumé et lien vers leur section de
    l'EBNF) ;
  - **Go-to-Definition** (Ctrl+Clic) : variables, paramètres, fonctions, méthodes (héritées comprises),
    champs, constantes, classes, lignes `import` / `runtime` / `wiring` (d'une interface ou d'une classe
    de test), argument nommé → paramètre ;
  - **autocomplétion** sur le type réel du receveur (`a.`, `self.`, `a.b().`, `A::`) : membres hérités,
    builtins, sucre `String`/`Array`/`Map` et conversions `Convert` sur un primitif, propriétés
    `message`/`code`/`source` d'une exception, noms visibles, classes après `use`, noms des paramètres
    restants dans un appel nommé — chaque appel complété insère ses paramètres comme champs à remplir ;
  - **aide à la signature**, paramètre actif par position ou par nom ;
  - **références** (Maj+F12) et **CodeLens** au-dessus des déclarations (implémentations, overrides,
    références), sur tout l'espace de travail : un appel via une sous-classe ou une interface compte pour
    la méthode déclarante ;
  - **symboles du document** (plan, `Ctrl+Shift+O`) ;
  - **renommage** (F2) d'une variable, d'un paramètre, d'un champ, d'une méthode, d'une classe ou d'une
    fonction dans tout l'espace de travail — imports et lignes `wiring` mis à jour, fichier renommé avec sa classe ; une
    méthode redéfinie est renommée avec toute sa chaîne (parents, sous-classes, interfaces).
  - **imports manquants** : un type utilisé sans import (`HTTPServer::…`, `use Foo()`, `x:Foo`, y
    compris dans un gabarit `${…}`) est importé à l'enregistrement (réglage `ocara.autoImport.enable`) :
    `import ocara.X` pour un builtin, sinon le fichier du projet qui le déclare, cherché dans le
    dossier du script, ses sous-dossiers, puis chaque dossier parent jusqu'à la racine du projet
    (`import a.b.Foo` si le chemin suit le namespace, sinon `import Foo from "./…"`). Correctif
    rapide (Ctrl+.) sur le nom, avec les autres déclarations homonymes du projet en alternative ;
  - **variantes `when`** : la variante écartée pour la configuration de l'éditeur (réglages
    `ocara.os`/`ocara.arch`/`ocara.build`) est estompée ; les diagnostics des autres configurations
    sont signalés avec leur combinaison (`[System::OS = 'windows']`) ; `when` et ses opérateurs
    ont une couleur dédiée, quel que soit le thème.
- **Barre latérale Ocara** (icône du logo dans la barre d'activité) :
  - **LED « Release »** en tête du panneau, avec trois voyants repris des encarts **Linter**,
    **Coverage** et **Architecture** (voir plus bas) : Linter sans avertissement, Coverage avec tous
    les tests au vert et la couverture par ligne au moins égale à `ocara.coverage.threshold` (100 %
    par défaut), Architecture sans erreur. Chaque voyant suit le périmètre de son encart ; un résultat
    ne compte que s'il porte sur la cible courante (même dossier ou même script affiché), sinon le
    voyant est « non mesuré ». Couleur selon la part de voyants au vert : rouge sous 33,33 %, orange
    jusqu'à 66,66 %, bleu jusqu'à 99,99 %, vert à 100 % — les boutons **Compiler** et **Lancer**
    passent alors au vert en BUILD `release`. Survol : détail et périmètre de chaque voyant ;
  - **Configuration** (en-tête fixe) : OS, ARCH (valeur personnalisée possible) et BUILD de l'éditeur
    (réglages `ocara.os`/`ocara.arch`/`ocara.build`), boutons **Compiler**, **Lancer** (Debug ou Release
    selon BUILD), **Fixer**, **Dump**, et accès aux paramètres de l'extension ;
  - **Navigation** : chaque fichier ouvert depuis le panneau (Workflow, graphe d'appels) entre dans un
    historique ; une barre **← / →** avec la liste des étapes apparaît en tête du panneau (et les
    flèches dans la barre de titre de l'éditeur) pour revenir aux scripts précédents. Ouvrir un autre
    fichier à la main ferme l'historique ;
  - **Workflow**, en partant du script affiché : arbre des **imports** (récursif, runtimes compris,
    doublons signalés), **structure** (classes, structs, interfaces, enums, fonctions, membres et
    variables locales avec leur type réel), **graphe d'appels** (fonctions et méthodes appelées,
    dépliable niveau par niveau, récursion signalée) et **variantes `when`** (retenue ou écartée pour
    la configuration de l'éditeur). Un clic ouvre la déclaration ; les imports, déclarations, membres,
    variables et appels d'une variante `when` écartée sont estompés ;
  - **Architecture hexagonale** (docs/EBNF.md §33) : le bloc `architecture` et le segment de couche des
    `namespace`/`import`/`wiring` (`context.car.**infra**.db`, selon les alias du projet) prennent la
    couleur des instructions bas niveau (comme `when`) ; l'encart Workflow indique la place du script
    (« car · infrastructure (infra) ») et la couche de chaque import ; le graphe d'appels range les boîtes
    dans un cadre jaune par couche, regroupé par contexte (cadres repris à l'export Mermaid) ;
  - **Graphe d'appels** interactif, à partir du script affiché (pour un `main.oc` : ses blocs runtime,
    fichiers runtime compris) : une boîte par type — class, struct, interface, module, generic,
    fonctions d'un fichier, bloc runtime — de couleur propre, listant les méthodes utilisées avec leurs
    paramètres et leur type de retour ; colonnes par niveau d'appel, ordonnées pour limiter les
    croisements. Liens **souples** (courbes) ou **droits** (en angle) ; molette = zoom, glisser = déplacer
    le fond ou une boîte, survol = appels mis en avant, clic = ouvrir la déclaration ; variantes `when`
    écartées estompées. Un appel écrit via une interface passe par sa méthode, reliée à l'implémentation
    visée par son `wiring` (lien violet en tirets) ; les appels builtin (`IO::writeln`, `Thread.run`…)
    sont regroupés par classe `ocara.*` en gris, masquables (bouton `ocara.*`). **Exporter** en Markdown (`.md`, graphe Mermaid) ou en image (`.png`), ou
    **copier** le Markdown dans le presse-papiers ;
  - **Benchmark** : temps et pic mémoire sur plusieurs exécutions (build Release), **détection des
    fuites** en un clic (`ocara --leak-check` : le script s'exécute tel quel, les valeurs non libérées
    sont listées par classe, puis par fonction d'allocation — un clic ouvre la fonction), fuite par croissance (marqueur `__N__`), profil par fonction (`ocara --profile`) et historique comparé par script
    (voir docs/benchmarking.md). Le délai `ocara.benchmark.timeout` tue le programme mesuré lui-même
    (tout son groupe de processus) ; un code de sortie non nul fait échouer la mesure de temps, mais le
    bilan de fuites et le profil restent affichés (fin anormale signalée). Les résultats restent
    attachés au script mesuré même si l'éditeur actif change pendant la mesure. Fuites (nombre de
    valeurs) et croissance (Ko) sont comparées séparément dans l'historique. Les copies `__N__` sont
    écrites dans un dossier temporaire, jamais dans le projet (imports résolus depuis le dossier du
    script par `--src`, sauf si `ocara.compilerPath` fixe déjà son propre `--src`).
  - **Périmètre et mode automatique des encarts Linter, Coverage et Architecture** : réglages
    `ocara.lint.scope`, `ocara.coverage.scope`, `ocara.architecture.scope` —
    - `workspace` : tout l'espace de travail ;
    - `folder` : le **dossier du script affiché**, sous-dossiers compris ;
    - `script` : le **script affiché** seul.

    Bouton ▶ de l'en-tête : analyse à la demande. Bouton ⟳ / ⊘ de l'en-tête : active ou désactive le
    **mode automatique** de l'encart (réglages `ocara.lint.auto`, `ocara.coverage.auto`,
    `ocara.architecture.auto`) — en `workspace`, à l'ouverture et à chaque enregistrement d'un `.oc` ;
    en `folder`, à l'enregistrement d'un `.oc` du dossier et quand le script affiché change de
    dossier ; en `script`, à l'enregistrement du script et quand le script affiché change.
  - **Coverage** : tests lancés par `ocaraunit --line-coverage --json` (binaires instrumentés par
    `ocara --coverage`), couverture **par ligne** fusionnée entre projets (un projet : dossier `tests`
    contenant des `*Test.oc`, la racine étant son parent — ou `ocara.coverage.projects`). Affiche la
    couverture, les tests en échec (un clic ouvre la sortie « Ocara »), puis par fichier les plages de
    lignes non couvertes : un clic ouvre le fichier et place le curseur au début de la plage. Ces lignes
    sont **surlignées en rouge** dans l'éditeur ; les fichiers entièrement couverts sont listés **en
    vert**. Selon le périmètre : `workspace`, tous les projets ; `folder`, tests du projet qui contient
    le dossier, couverture limitée au dossier (un dossier de tests ne lance que ses tests, un dossier
    parent de projets les couvre entièrement) ; `script`, tests du projet du script, couverture limitée
    au script (un `*Test.oc` ne lance que lui-même). Exclusions : `ocara.coverage.exclude`.
  - **Linter** : avertissements d'`ocaracs` regroupés par fichier ; un clic ouvre le fichier sur
    l'avertissement. `workspace` : projets de l'espace de travail (racines des projets testés, sinon
    dossiers de l'espace de travail) ; `folder` : dossier du script affiché ; `script` : le script
    affiché, ses propres avertissements seulement.
  - **Architecture** : vérification de l'architecture hexagonale par le compilateur (`ocara <fichier>
    --check-architecture`, docs/EBNF.md §33) — couche de chaque fichier selon son chemin
    `<src>/<contexte>/<couche>/…`, sens des dépendances entre couches, isolement des contextes,
    `wiring` et bibliothèques externes, avec les règles du bloc `architecture` du fichier, sinon du
    `main.oc` du projet ; chaque fichier est vérifié avec tout ce qu'il importe. `workspace` : chaque
    `main.oc` de l'espace de travail ; `folder` : chaque script du dossier du script affiché ;
    `script` : le script affiché (tests et blocs runtime exclus, ainsi que `ocara.coverage.exclude`).
    Erreurs regroupées par fichier, un clic ouvre le fichier sur l'erreur.
  - Chaque bloc peut être déplacé hors de la barre latérale (panneau du bas, autre barre) : son onglet
    porte alors son nom suivi d'« Ocara » (« Coverage - Ocara », « Linter - Ocara », « Architecture - Ocara »…).
- Les noms de type (`var x:Dog`, `extends Animal`, paramètres, `on e is FileException`) ont survol,
  définition et références comme les autres noms.
- **Documentation embarquée** : `docs/EBNF.md` et `docs/builtins/*.md` du dépôt sont copiés dans
  l'extension à chaque compilation (`scripts/copy-docs.js`, lancé par `npm run compile` ; dossier
  `docs/` de l'extension ignoré par git, ne jamais l'éditer). Le lien « 📖 » d'une popup de survol ouvre
  le fichier en **aperçu** Markdown, sur la section concernée.
- Catalogue des builtins (`data/builtins-data.json`, généré depuis `src/builtins/*.rs` et
  `docs/builtins/*.md` par `scripts/generate-builtins-data.py` — à relancer après tout ajout/changement de
  builtin ou de sa doc) : embarqué dans le compilateur pour le survol et la complétion.

- **Analyse de style `ocaracs` automatique** à l'affichage, à l'ouverture et à l'enregistrement
  d'un fichier `.oc` : chaque ligne concernée est surlignée en jaune, le message s'affiche au
  survol (et dans le panneau Problèmes). Règles : le `.ocaracs` le plus proche en remontant
  depuis le dossier du script, sinon les valeurs par défaut d'ocaracs. ocaracs lisant le fichier
  sur disque, l'analyse porte sur la dernière version enregistrée. Les demandes rapprochées
  (ouverture + affichage) sont regroupées en une analyse ; le résultat d'une analyse dépassée
  (fichier fermé ou ré-analysé entre-temps) est ignoré ; désactiver `ocara.lint.enable` efface
  diagnostics et surlignage.
- **Clic droit → « Compiler (Debug) » / « Compiler (Release) »** (éditeur ou arborescence,
  aussi dans la palette) : demande le nom du binaire, créé dans le dossier du script (le
  compilateur y laisse aussi `<nom>.o`) ; Release compile avec `--release` (`System::BUILD`
  vaut `release`). Erreurs dans le panneau Problèmes et la sortie « Ocara » ; celles d'un fichier
  disparaissent dès qu'il est modifié ou enregistré.
- **Clic droit → « Compiler et lancer (Debug) » / « (Release) »** : demande le nom du binaire
  comme « Compiler », puis l'exécute **depuis le dossier du script** dans le terminal intégré « Ocara »
  (entrée clavier et programmes longs, comme un serveur, fonctionnent). Le binaire est le processus
  même du terminal (aucun shell : fonctionne avec bash, PowerShell ou cmd) ; un nouveau lancement
  ferme le terminal « Ocara » précédent et le programme qui y tournait.
- **Clic droit → « Afficher le dump »** : tokens, AST et IR (`ocara --dump`) dans un éditeur
  sans fichier — le binaire que `--dump` produit malgré tout est compilé dans un dossier
  temporaire supprimé aussitôt.
- **Clic droit → « Fixer la mise en forme »** (sur un script, ou sur un dossier dans
  l'arborescence) : `ocaracs --fix` (indentation, espaces, lignes vides, newline finale,
  nommage). Les scripts modifiés sont d'abord enregistrés. Si des identifiants doivent être
  renommés (déclaration et usages dans tout le projet), une confirmation liste ces renommages.
  Un fichier renommé avec sa classe est rouvert sous son nouveau nom, l'analyse est relancée,
  et le compte rendu complet est dans la sortie « Ocara ».

## Réglages

| Réglage | Défaut | Rôle |
|---------|--------|------|
| `ocara.compilerPath` | `ocara` | Chemin du compilateur ou commande pour le lancer — arguments et guillemets acceptés, variables `${workspaceFolder}`/`${fileDirname}` (ex. `"${workspaceFolder}/target/release/ocara"`). |
| `ocara.ocaracsPath` | `ocaracs` | Chemin d'ocaracs ou commande pour le lancer (mêmes règles). |
| `ocara.ocaraunitPath` | `ocaraunit` | Chemin d'ocaraunit ou commande pour le lancer (mêmes règles). Le compilateur de `ocara.compilerPath` lui est transmis (`OCARA`). |
| `ocara.lint.enable` | `true` | Active l'analyse ocaracs automatique. |
| `ocara.lint.scope` | `workspace` | Périmètre de l'encart Linter : `workspace`, `folder` (dossier du script affiché) ou `script` (script affiché). |
| `ocara.lint.auto` | `true` | Mode automatique de l'encart Linter (bouton de l'en-tête). |
| `ocara.coverage.scope` | `workspace` | Périmètre de l'encart Coverage : `workspace`, `folder` (dossier du script affiché) ou `script` (script affiché). |
| `ocara.coverage.auto` | `true` | Mode automatique de l'encart Coverage (bouton de l'en-tête). |
| `ocara.coverage.exclude` | `[]` | Dossiers ou fichiers exclus de la couverture et des voyants (`core`, `main.oc`, `context/car/infra/db`), comparés par segments de chemin depuis la racine de chaque projet. |
| `ocara.coverage.projects` | `[]` | Dossiers de tests à lancer, relatifs à l'espace de travail ; vide : détection automatique. |
| `ocara.coverage.threshold` | `100` | Couverture (%) à partir de laquelle le voyant Coverage est vert. |
| `ocara.architecture.scope` | `workspace` | Périmètre de l'encart Architecture : chaque `main.oc` (`workspace`), chaque script du dossier du script affiché (`folder`) ou le script affiché (`script`). |
| `ocara.architecture.auto` | `true` | Mode automatique de l'encart Architecture (bouton de l'en-tête). |
| `ocara.benchmark.runs` | `5` | Nombre d'exécutions de « Mesurer temps et mémoire ». |
| `ocara.benchmark.timeout` | `30` | Délai maximal d'une exécution mesurée (secondes) ; au-delà, le programme est tué. |
| `ocara.benchmark.leakSizes` | `[20000, 200000]` | Valeurs de `__N__` comparées par la détection de fuite. |
| `ocara.autoImport.enable` | `true` | Ajoute les imports manquants à l'enregistrement (le correctif rapide reste disponible sinon). |
| `ocara.os` | `system` | `System::OS` de l'éditeur pour les clauses `when` : `system` (machine), `linux`, `windows`, `macos`, `android`, ou `other` (valeur de `ocara.osOther`). Variante retenue, variantes écartées estompées. Ne change pas la cible des commandes de compilation. |
| `ocara.osOther` | `""` | Valeur personnalisée de `System::OS` (avec `ocara.os` = `other`). |
| `ocara.arch` | `system` | `System::ARCH` de l'éditeur : `system`, `x86_64`, `aarch64`, `arm`, ou `other` (valeur de `ocara.archOther`). |
| `ocara.archOther` | `""` | Valeur personnalisée de `System::ARCH` (avec `ocara.arch` = `other`). |
| `ocara.build` | `debug` | `System::BUILD` de l'éditeur : `debug` ou `release`. Les commandes de compilation gardent leur propre mode (Debug/Release). |

Laissé à sa valeur par défaut et absent du PATH, un outil est cherché dans
`<workspace>/target/release/` (dépôt du compilateur ouvert dans VS Code).

---

## Installation du `.vsix` pré-compilé

> Prérequis : VS Code ≥ 1.85

```bash
code --install-extension ocara-language-1.0.0.vsix
```

Rechargez VS Code : `Ctrl+Shift+P` → **Reload Window**.

---

## Construire et installer depuis les sources

> Prérequis : Node.js ≥ 18, npm

**1. Installer les dépendances**

```bash
cd tools/highlight/vsode
npm install
```

**2. Compiler le TypeScript**

```bash
npm run compile
```

**3. Packager l'extension**

```bash
npx vsce package --allow-missing-repository
```

Cela génère `ocara-language-1.0.0.vsix` dans le répertoire courant.

**4. Installer l'extension**

```bash
code --install-extension ocara-language-1.0.0.vsix
```

**5. Recharger VS Code**

`Ctrl+Shift+P` → **Reload Window**

---

## Désinstallation

**Via la ligne de commande**

```bash
code --uninstall-extension david-lhoumaud.ocara-language
```

**Via l'interface VS Code**

`Ctrl+Shift+X` → rechercher *Ocara* → **Uninstall**

**En cas d'erreur "Please restart VS Code before reinstalling"**

L'extension peut laisser une entrée fantôme dans le registre interne. Pour nettoyer manuellement :

```bash
# Supprimer le dossier d'installation
rm -rf ~/.vscode/extensions/david-lhoumaud.ocara-language-*

# Supprimer l'entrée du registre
python3 -c "
import json
with open('/home/$USER/.vscode/extensions/extensions.json') as f: data=json.load(f)
cleaned=[e for e in data if 'ocara' not in e.get('identifier',{}).get('id','').lower()]
with open('/home/$USER/.vscode/extensions/extensions.json','w') as f: json.dump(cleaned,f,indent=2)
print(f'Removed {len(data)-len(cleaned)} entries')
"
```

Puis relancez `code --install-extension ocara-language-1.0.0.vsix`.

---

## Mise à jour

```bash
cd tools/highlight/vsode
code --uninstall-extension david-lhoumaud.ocara-language
npm install
npm run compile
npx vsce package --allow-missing-repository
code --install-extension ocara-language-1.0.0.vsix
```

Après modification de la grammar ou du provider, relancez les étapes 2 à 5.

Pour incrémenter la version, modifiez le champ `"version"` dans `package.json` avant l'étape 3.

