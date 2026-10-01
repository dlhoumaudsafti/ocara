# Ocara Language — Extension VS Code

Coloration syntaxique, autocomplétion et navigation (Go-to-Definition) pour le langage **Ocara** (`.oc`).

## Fonctionnalités

- Highlight complet : mots-clés, types, classes, structs, méthodes, imports, chaînes, templates
- Appels de méthodes (`obj.method()`), accès statiques (`Class::member`), builtins `ocara.*`
- **Autocomplétion après `.` / `::`** — méthodes et constantes des classes builtin `ocara.*`
  (catalogue généré depuis `src/builtins/*.rs`, voir `data/builtins-data.json`) et des
  classes utilisateur (propres et héritées via `extends`, résolues à travers les imports)
- **Autocomplétion `self.` / `self::` / `parent.`** dans le corps d'une classe
- **Autocomplétion sur une variable de type primitif** (`string`, `int`, `float`, `bool`, `array<T>`,
  `map<K,V>`) : conversions (`s.toInt()`, `n.toStr()`, `arr.toStr(sep)`... ≡ `Convert::*`, voir
  `docs/builtins/Convert.md`) et méthodes `String`/`Array`/`Map` utilisables en instance (`s.trim()`),
  avec signature help
- **Autocomplétion `e.message` / `e.code` / `e.source`** dans un bloc `on e is XException`
- **Autocomplétion des noms de classe après `use `** (builtins instanciables + classes utilisateur)
- **Arguments nommés** (`use UserDto(id: 42, name: 'David')`) :
  - coloration du nom d'argument (`nom:` dans un appel) ;
  - autocomplétion des noms de paramètres dans les parenthèses d'un appel (fonction, méthode,
    `Classe::méthode`, `use Classe(...)` → `init`, méthodes statiques builtin), sans les noms
    déjà fournis ni le variadic, et jamais après un argument positionnel (un appel est soit
    100 % positionnel, soit 100 % nommé) ;
  - **signature help** (`(` / `,`) : signature complète, valeurs par défaut affichées,
    paramètre actif déterminé par nom pour un argument nommé, par position sinon ;
  - **Ctrl+Click** sur `nom:` → paramètre correspondant dans la déclaration.
  - `use MonStruct(` : signature du constructeur **généré** depuis les champs du `struct`
    (champs hérités d'abord, valeurs par défaut affichées), Ctrl+Click sur `nom:` → le champ.
  - Limites : aucune aide pour un appel via une valeur `Function<...>` (les noms de paramètres
    n'y existent pas) ni pour un receveur sans type déclaré ; une valeur ressemblant à un type
    (`p: Point`, `c: CONST`) n'est pas colorée comme argument nommé (indiscernable de `nom:Type`).
- **Ctrl+Click** sur un `import` → ouvre le fichier `.oc` correspondant
- **Ctrl+Click** sur `import Circle from "11_interfaces"` → ouvre le fichier et positionne sur la classe `Circle`
- **Ctrl+Click** sur `self.circle.area()` → navigue vers la méthode `area()` dans la classe importée
- **Ctrl+Click** sur `ClassName::member` → ouvre le fichier de la classe et positionne le curseur sur la méthode
- **Ctrl+Click** sur un nom de variable ou fonction → navigue vers la déclaration
- **Scan automatique du workspace** pour résoudre les imports `from "file"` dans n'importe quel sous-dossier
- **Namespaces à plusieurs segments** (`namespace context.search.app.usecase`) : un import
  `context.search.domain.contract.SearchContract` est résolu depuis la racine du projet déduite
  du namespace (puis depuis chaque dossier parent), et Ctrl+Click positionne sur la déclaration
  de la classe elle-même
- **CodeLens** au-dessus de chaque déclaration, calculés sur tout le workspace (index construit à
  l'activation, mis à jour à chaque modification), cliquables pour lister les emplacements :

  | Déclaration      | CodeLens |
  |------------------|----------|
  | classe / generic | implémentations (sous-classes, transitif) · overrides (méthodes redéfinies par ces sous-classes) |
  | struct           | implémentations (structs dérivés, transitif) |
  | interface        | implémentations (classes qui l'implémentent, héritage compris) · overrides (méthodes de l'interface qu'elles définissent) |
  | module           | implémentations (classes qui l'utilisent via `modules`) · overrides (méthodes du module qu'elles redéfinissent) |
  | méthode          | d'interface : implémentations · de module : implémentations + overrides · de classe : overrides |
  | fonction / enum  | références |

  Résolution par nom, sans suivre les imports : deux classes homonymes de contextes différents
  sont confondues.

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

