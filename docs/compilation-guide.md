# Ocara — Guide de compilation

> Guide pratique pour compiler un script `.oc` avec le compilateur `ocara`.

---

## Prérequis

| Outil | Version minimale | Rôle |
|-------|-----------------|------|
| Rust / Cargo | 1.75+ | Construire le compilateur |
| `cc` (gcc ou clang) | tout | Éditeur de liens final |

Le compilateur produit un **binaire natif** via Cranelift (backend codegen) + `cc` pour la liaison.

---

## 1. Construire le compilateur

```bash
# Dans le répertoire racine du projet
make build
```

Compile le runtime (`libocara_runtime.a`) et le compilateur. Le runtime est **embarqué dans le binaire** — un seul fichier autonome est produit :

```
target/release/ocara
```

Vous pouvez l'ajouter à votre `PATH` pour l'utiliser globalement :

```bash
export PATH="$PATH:/chemin/vers/Ocara/target/release"
```

Ou l'installer globalement :

```bash
make install   # copie ocara dans /usr/local/bin/
```

---

## 2. Compiler un script `.oc`

### Syntaxe générale

```
ocara [OPTIONS] [FICHIER]
```

| Argument | Description |
|----------|-------------|
| `FICHIER` | Chemin vers le script source `.oc` (défaut : `test.oc`) |
| `-o NOM` | Nom du binaire de sortie (défaut : `out`) |
| `--check` | Analyse sémantique uniquement — n'émet aucun binaire |
| `--no-link` | Génère le fichier objet `.o` sans lier |
| `--dump` | Affiche les tokens, l'AST et le HIR puis s'arrête |

---

### Compilation standard

```bash
# Compiler hello.oc → binaire ./hello
ocara hello.oc -o hello

# Exécuter le binaire produit
./hello
```

---

### Vérification sémantique uniquement

Utile pour détecter les erreurs de types et de symboles sans produire de binaire :

```bash
ocara hello.oc --check
# check ok — aucune erreur sémantique.
```

En cas d'erreur :

```
semantic error: [1:5] symbole indéfini 'foo'
```

---

### Produire un fichier objet sans lier

```bash
ocara hello.oc --no-link -o hello
# → génère hello.o
```

Vous pouvez ensuite lier manuellement avec vos propres bibliothèques :

```bash
cc hello.o -o hello -lm
```

---

### Mode diagnostic (dump)

Affiche le flux interne du compilateur sans rien écrire sur le disque :

```bash
ocara hello.oc --dump
```

Sortie typique :

```
=== TOKENS (42) ===
...

=== AST ===
Program { imports: [...], classes: [...], functions: [...] }

=== HIR (3 fonctions) ===
func main (2 blocs)
  bb0:
    ConstStr { dest: v0, idx: 0 }
    Call { dest: Some(v1), func: "print", ... }
    Jump { target: bb1 }
  bb1:
    Return { value: Some(v2) }
```

---

## 3. Pipeline de compilation interne

Le compilateur applique les étapes suivantes dans l'ordre :

```
Source .oc
  │
  ▼
Lexer           → tokens
  │
  ▼
Parser          → AST (arbre syntaxique abstrait)
  │
  ▼
SymbolTable     → enregistrement des symboles globaux
  │
  ▼
TypeChecker     → vérification sémantique et de types
  │
  ▼
Lowering        → Ocara HIR (représentation intermédiaire)
  │
  ▼
Cranelift       → fichier objet natif (.o)
  │
  ▼
cc (linker)     → binaire exécutable final
```

---

## 5. Exemple complet

Fichier `hello.oc` :

```ocara
import ocara.IO

function main(): int {
    IO::writeln("Bonjour depuis Ocara !")
    return 0
}
```

Compilation et exécution :

```bash
make build
./target/release/ocara hello.oc -o hello
./hello
# Bonjour depuis Ocara !
```

---

## 6. Bibliothèque standard runtime

Les classes de la bibliothèque standard runtime sont disponibles via `import ocara.<Classe>` ou `import ocara.*`.

| Classe | Import | Description |
|--------|--------|-------------|
| `IO` | `import ocara.IO` | `IO::writeln(val)`, `IO::read()` — entrées/sorties standard |
| `Math` | `import ocara.Math` | Fonctions mathématiques et constantes (`Math::PI`, `Math::sqrt`, …) |
| `String` | `import ocara.String` | Manipulation de chaînes (`split`, `trim`, `replace`, …) |
| `Array` | `import ocara.Array` | Manipulation de tableaux (`push`, `pop`, `sort`, …) |
| `Map` | `import ocara.Map` | Manipulation de maps (`keys`, `values`, `has`, …) |
| `JSON` | `import ocara.JSON` | Encodage/décodage JSON (`encode`, `decode`, `pretty`, `minimize`) |
| `Convert` | `import ocara.Convert` | Conversions entre types (`toInt`, `toString`, …) |
| `Regex` | `import ocara.Regex` | Expressions régulières (POSIX ERE) |
| `System` | `import ocara.System` | OS, PID, env, exec, args (`System::OS`, `System::ARCH`) |
| `Thread` | `import ocara.Thread` | Threads natifs (`spawn`, `join`, `sleep`) |
| `HTTPRequest` | `import ocara.HTTPRequest` | Requêtes HTTP/HTTPS (GET, POST, PUT, DELETE, PATCH) |
| `HTTPServer` | `import ocara.HTTPServer` | Serveur HTTP simple (`listen`, `route`, `response`) |
| `UnitTest` | `import ocara.UnitTest` | Assertions pour tests unitaires (`assertEquals`, `assertTrue`, …) |

> **Note :** Certaines méthodes de `String`, `Array`, `Map` et `JSON` sont disponibles comme **méthodes d'instance** sans import :
> - `array.encode()`, `map.encode()` — convertir en JSON
> - `string.decode()`, `string.pretty()`, `string.minimize()` — opérations JSON
> - `string.split()`, `string.trim()` — manipulation de chaînes
> - `array.push()`, `array.pop()` — manipulation de tableaux

### Exemple

```ocara
import ocara.IO
import ocara.JSON

function main(): int {
    IO::writeln("Quel est ton nom ?")
    var nom:string = IO::read()
    IO::writeln("Bonjour " + nom)
    
    // Utiliser JSON
    var data:map = map("nom": nom, "age": 25)
    var json:string = JSON::encode(data)
    IO::writeln("JSON: " + json)
    
    return 0
}
```

Voir `examples/builtins/` pour des exemples complets de chaque classe builtin.

---

## 7. Messages d'erreur courants

| Message | Cause |
|---------|-------|
| `error: impossible de lire 'fichier.oc'` | Le fichier source n'existe pas ou n'est pas lisible |
| `lexer error: unexpected char '@'` | Caractère non reconnu dans le source |
| `parse error: expected ')'` | Parenthèse ou accolade manquante |
| `semantic error: symbole indéfini 'X'` | Utilisation d'une variable/fonction non déclarée |
| `semantic error: type attendu 'int', trouvé 'string'` | Incompatibilité de types |
| `codegen error: ...` | Erreur interne Cranelift |

| `link error: ...` | `cc` introuvable ou erreur d'édition de liens |

---

## 8. Structure d'un projet Ocara

Un projet Ocara est un dossier contenant des fichiers `.oc`. Le point d'entrée est la fonction `main` :

```
mon-projet/
├── main.oc          ← point d'entrée (function main(): int)
├── classes/
│   ├── Models.oc
│   ├── Services.oc
│   └── Utils.oc
└── tests/           ← tests unitaires (*Test.oc)
    ├── ModelsTest.oc
    ├── ServicesTest.oc
    └── UtilsTest.oc
```

Le compilateur suit les imports automatiquement — compiler le point d'entrée suffit :

```bash
./target/release/ocara main.oc -o mon-projet
```

### Tests unitaires

Pour lancer les tests avec `ocaraunit` :

```bash
# Installer ocaraunit
make build-tools
make install-tools

# Lancer tous les tests
ocaraunit

# Avec couverture
ocaraunit --coverage

# Test spécifique
ocaraunit tests/ModelsTest.oc
```

Voir `tools/ocaraunit/Readme.md` pour la documentation complète.

Voir `examples/project/` pour un exemple complet multi-fichiers avec tests.

---

## 9. Options de débogage du compilateur lui-même

```bash
# Build debug (plus lent, symboles de débogage inclus)
cargo build

# Lancer les tests unitaires
make test

# Régression complète
make regression

# Un seul exemple
make regression 07_loops

# Tests unitaires du projet
make unittest

# Tests avec couverture
make unittest-coverage
```

---

## 10. Outils complémentaires

### ocaraunit — Runner de tests

Exécute automatiquement les fichiers `*Test.oc` et génère un rapport de couverture.

```bash
ocaraunit                    # tous les tests dans tests/
ocaraunit --coverage         # avec analyse de couverture
ocaraunit tests/MyTest.oc    # un test spécifique
```

Voir `tools/ocaraunit/Readme.md`

### ocaracs — Analyseur de style

Vérifie le respect des conventions de codage Ocara.

```bash
ocaracs src/               # analyser un dossier
ocaracs main.oc            # analyser un fichier
```

Voir `tools/ocaracs/Readme.md`
