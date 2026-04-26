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

## 6. Classes builtins

Les classes builtins sont disponibles via `import ocara.<Classe>` ou `import ocara.*`.

| Classe | Import | Description |
|--------|--------|-------------|
| `IO` | `import ocara.IO` | `IO::writeln(val)`, `IO::read()` — entrées/sorties standard |
| `Math` | `import ocara.Math` | Fonctions mathématiques et constantes (`Math::PI`, `Math::sqrt`, …) |
| `String` | `import ocara.String` | Manipulation de chaînes |
| `Array` | `import ocara.Array` | Manipulation de tableaux |
| `Map` | `import ocara.Map` | Manipulation de maps |
| `Convert` | `import ocara.Convert` | Conversions entre types |
| `Regex` | `import ocara.Regex` | Expressions régulières (POSIX ERE) |
| `System` | `import ocara.System` | OS, PID, env, exec, args, `System::OS`, `System::ARCH` |
| `HTTPRequest` | `import ocara.HTTPRequest` | Requêtes HTTP/HTTPS (GET, POST, PUT, DELETE, PATCH) |

> **Note :** `write()` et `read()` sans import sont dépréciés. Utiliser `IO::writeln()` et `IO::read()`.

### Exemple

```ocara
import ocara.IO

function main(): int {
    IO::writeln("Quel est ton nom ?")
    var nom:string = IO::read()
    IO::writeln("Bonjour " + nom)
    return 0
}
```

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
├── models/
│   └── User.oc
└── services/
    └── Logger.oc
```

Le compilateur suit les imports automatiquement — compiler le point d'entrée suffit :

```bash
./target/release/ocara main.oc -o mon-projet
```

Voir `examples/project/` pour un exemple complet multi-fichiers.

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
```
