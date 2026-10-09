# Ocara Language — Extension VS Code

Coloration syntaxique, et via le serveur de langage du compilateur : diagnostics en direct, autocomplétion, survol, navigation, références et CodeLens pour le langage **Ocara** (`.oc`).

## Fonctionnalités

- **Coloration syntaxique** complète (grammaire TextMate `syntaxes/ocara.tmLanguage.json`) : mots-clés,
  types, classes, structs, méthodes, imports, chaînes, templates, noms d'arguments nommés.
- **Serveur de langage du compilateur** (`ocara --lsp`, lancé avec le compilateur du réglage
  `ocara.compilerPath`) — toute la sémantique vient du compilateur lui-même, aucune heuristique :
  - **erreurs et avertissements pendant la frappe**, y compris ceux d'un fichier importé (signalés en
    tête du document, avec un lien) ; une erreur de syntaxe n'empêche pas le reste du document de
    fonctionner (toutes les erreurs de syntaxe sont signalées, survol et complétion continuent) ;
  - **survol** : type réel d'une variable, signature et commentaires `//` d'une déclaration,
    documentation des méthodes builtin (extraite de `docs/builtins/*.md`), méthode réellement appelée
    par le sucre d'instance (`s.trim()`, `s.toInt()`), mots-clés (résumé et lien vers leur section de
    l'EBNF) ;
  - **Go-to-Definition** (Ctrl+Clic) : variables, paramètres, fonctions, méthodes (héritées comprises),
    champs, constantes, classes, lignes `import` / `runtime` / `wiring`, argument nommé → paramètre ;
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
    fonction dans tout l'espace de travail — imports mis à jour et fichier renommé avec sa classe ; une
    méthode redéfinie est renommée avec toute sa chaîne (parents, sous-classes, interfaces).
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
  sur disque, l'analyse porte sur la dernière version enregistrée.
- **Clic droit → « Compiler le script »** (éditeur ou arborescence, aussi dans la palette
  `Ocara: Compiler le script`) : demande le nom du binaire, créé dans le dossier du script
  (le compilateur y laisse aussi `<nom>.o`). Erreurs dans le panneau Problèmes et la sortie
  « Ocara ».
- **Clic droit → « Compiler et lancer »** : demande le nom du binaire comme « Compiler le
  script », puis l'exécute **depuis le dossier du script** dans le terminal intégré « Ocara »
  (entrée clavier et programmes longs, comme un serveur, fonctionnent). Un seul terminal
  « Ocara » sert à tous les lancements : le programme précédent y est arrêté (Ctrl+C) avant.
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
| `ocara.lint.enable` | `true` | Active l'analyse ocaracs automatique. |

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

