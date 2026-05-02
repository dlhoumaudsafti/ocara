# Ocara Language — Extension VS Code

Coloration syntaxique et navigation (Go-to-Definition) pour le langage **Ocara** (`.oc`).

## Fonctionnalités

- Highlight complet : mots-clés, types, classes, méthodes, imports, chaînes, templates
- Appels de méthodes (`obj.method()`), accès statiques (`Class::member`), builtins `ocara.*`
- **Ctrl+Click** sur un `import` → ouvre le fichier `.oc` correspondant
- **Ctrl+Click** sur `import Circle from "11_interfaces"` → ouvre le fichier et positionne sur la classe `Circle`
- **Ctrl+Click** sur `self.circle.area()` → navigue vers la méthode `area()` dans la classe importée
- **Ctrl+Click** sur `ClassName::member` → ouvre le fichier de la classe et positionne le curseur sur la méthode
- **Ctrl+Click** sur un nom de variable ou fonction → navigue vers la déclaration
- **Scan automatique du workspace** pour résoudre les imports `from "file"` dans n'importe quel sous-dossier

---

## Installation du `.vsix` pré-compilé

> Prérequis : VS Code ≥ 1.85

```bash
code --install-extension ocara-language-0.1.0.vsix
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

Cela génère `ocara-language-0.1.0.vsix` dans le répertoire courant.

**4. Installer l'extension**

```bash
code --install-extension ocara-language-0.1.0.vsix
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

Puis relancez `code --install-extension ocara-language-0.1.0.vsix`.

---

## Mise à jour

Après modification de la grammar ou du provider, relancez les étapes 2 à 5.

Pour incrémenter la version, modifiez le champ `"version"` dans `package.json` avant l'étape 3.

