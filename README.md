# Ocara

**O**bject **C**ode **A**bstraction **R**untime **A**rchitecture

Langage de programmation compilé natif, statiquement typé, orienté objet.  
Extension des fichiers source : `.oc`

![Logo Ocara](logo.png)

---

## Prérequis

| Outil | Version minimale | Rôle |
|---|---|---|
| Rust / Cargo | 1.75+ | Construire le compilateur |
| `cc` (gcc ou clang) | tout | Éditeur de liens final |

`cc` est l'éditeur de liens appelé à chaque compilation d'un fichier `.oc`. Il doit être présent sur le système :

| Plateforme | Installation |
|---|---|
| Debian / Ubuntu | `sudo apt install build-essential` |
| Fedora / RHEL | `sudo dnf install gcc` |
| macOS | `xcode-select --install` |

---

## Compiler

```bash
make build
```

Compile le runtime (`libocara_runtime.a`) et le compilateur. Le binaire est produit dans `target/release/ocara`.

Optionnel — l'ajouter au `PATH` :

```bash
export PATH="$PATH:$(pwd)/target/release"
```

---

## Commandes Makefile

| Commande | Description |
|---|---|
| `make build` | Compile le runtime + le compilateur |
| `make test` | Lance les tests unitaires Cargo |
| `make regression` | Régression complète (tous les exemples) |
| `make regression <chemin>` | Un seul exemple, ex : `make regression 07_loops` |
| `make all` | `build` + `test` + `regression` |
| `make clean` | Supprime les artefacts de compilation |
| `make install` | Installe `ocara` dans `/usr/local/bin/` (binaire autonome) |
| `make uninstall` | Supprime `ocara` de `/usr/local/bin/` |

Résultat attendu de `make test` :

```
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

---

## Vérification sémantique d'un fichier `.oc`

La vérification sémantique analyse les types et les symboles sans produire de binaire.

```bash
./target/release/ocara mon_fichier.oc --check
```

Résultat attendu si le fichier est valide :

```
check ok — aucune erreur sémantique.
```

Pour vérifier tous les exemples, utiliser `make regression`.

---

## Compiler un fichier `.oc` en binaire

```bash
./target/release/ocara mon_fichier.oc -o mon_binaire
./mon_binaire
```

---

## Options du compilateur

| Option | Description |
|---|---|
| `-o NOM` | Nom du binaire de sortie (défaut : `out`) |
| `--check` | Vérification sémantique uniquement, sans produire de binaire |
| `--no-link` | Produit le fichier objet `.o` sans lier |
| `--dump` | Affiche les tokens, l'AST et le HIR puis s'arrête |

---

## Utiliser un fichier Ocara depuis un autre

### Situation actuelle

Le compilateur Ocara v0.1.0 ne dispose pas encore d'un mode bibliothèque natif (`--lib`, `.a`, `.so`).  
La seule option disponible est `--no-link` qui produit un fichier objet `.o` brut.

Il est possible de lier manuellement plusieurs `.o` ensemble via `cc` :

```bash
# Compiler chaque module en .o
./target/release/ocara utils.oc --no-link -o utils.o
./target/release/ocara main.oc  --no-link -o main.o

# Lier les deux en un seul binaire
cc utils.o main.o -o mon_programme -lm -no-pie
./mon_programme
```

> **Limite :** les symboles exportés par `utils.oc` (fonctions, classes) ne sont pas
> automatiquement visibles dans `main.oc` au moment de la vérification sémantique.
> Le système de modules multi-fichiers (résolution inter-fichiers à la compilation)
> est prévu dans une version future.

### Approche recommandée aujourd'hui : `import`

Pour partager du code entre fichiers, utiliser le système d'import existant.  
Le compilateur résout les imports à la compilation depuis la racine du projet :

```
projet/
├── utils.oc
├── models.oc
└── main.oc
```

```ocara
// main.oc
import utils
import models

function main(): int {
    // utilise les symboles de utils.oc et models.oc
    return 0
}
```

Compiler le point d'entrée suffit — le compilateur suit les imports :

```bash
./target/release/ocara main.oc -o mon_programme
./mon_programme
```

Voir `examples/project/` pour un exemple complet multi-fichiers.

---

## Structure du projet

```
Ocara/
├── src/
│   ├── main.rs            ← point d'entrée du compilateur
│   ├── lexer.rs           ← tokenisation
│   ├── token.rs           ← définition des tokens
│   ├── parser.rs          ← parsing et construction de l'AST
│   ├── ast.rs             ← types de l'AST
│   ├── builtins/          ← classes builtins (ocara.*)
│   │   ├── mod.rs
│   │   ├── array.rs
│   │   ├── convert.rs
│   │   ├── http.rs
│   │   ├── io.rs
│   │   ├── map.rs
│   │   ├── math.rs
│   │   ├── regex.rs
│   │   ├── string.rs
│   │   └── system.rs
│   ├── sema/              ← analyse sémantique et typage
│   ├── ir/                ← représentation intermédiaire (HIR)
│   └── codegen/           ← émission de code via Cranelift
├── examples/              ← exemples de code Ocara
│   ├── builtins/          ← exemples par classe builtin
│   └── project/           ← exemple de projet multi-fichiers
└── docs/                  ← documentation
    ├── EBNF.md            ← spécification du langage
    ├── compilation-guide.md
    └── builtins/          ← documentation des classes builtins
```

---

## Classes builtins disponibles

Importables via `import ocara.NomClasse` ou `import ocara.*`.

| Classe | Rôle |
|---|---|
| `Array` | Manipulation de tableaux |
| `Convert` | Conversions entre types |
| `HTTPRequest` | Requêtes HTTP/HTTPS |
| `IO` | Entrées / sorties standard (`IO::writeln`, `IO::read`) |
| `Map` | Manipulation de maps |
| `Math` | Fonctions et constantes mathématiques |
| `Regex` | Expressions régulières (POSIX ERE) |
| `String` | Manipulation de chaînes |
| `System` | Interaction avec le système d'exploitation |

---

## Exemple minimal

```ocara
import ocara.IO

function main(): int {
    IO::writeln("Bonjour depuis Ocara !")
    return 0
}
```

```bash
make build
./target/release/ocara hello.oc -o hello
./hello
# Bonjour depuis Ocara !
```

---

## Documentations

| Document | Description |
|---|---|
| [docs/EBNF.md](docs/EBNF.md) | **Spécification formelle du langage** — grammaire complète (EBNF) |
| [docs/compilation-guide.md](docs/compilation-guide.md) | Guide de compilation — options, pipeline, messages d'erreur |
| [docs/builtins/IO.md](docs/builtins/IO.md) | Référence `IO` — `writeln`, `read`, `read_int`, … |
| [docs/builtins/Math.md](docs/builtins/Math.md) | Référence `Math` — `sqrt`, `pow`, constantes `PI`, `E`, … |
| [docs/builtins/String.md](docs/builtins/String.md) | Référence `String` — `split`, `replace`, `trim`, … |
| [docs/builtins/Array.md](docs/builtins/Array.md) | Référence `Array` — `push`, `pop`, `sort`, `slice`, … |
| [docs/builtins/Map.md](docs/builtins/Map.md) | Référence `Map` — `get`, `set`, `keys`, `merge`, … |
| [docs/builtins/Convert.md](docs/builtins/Convert.md) | Référence `Convert` — conversions entre tous les types |
| [docs/builtins/Regex.md](docs/builtins/Regex.md) | Référence `Regex` — `test`, `find`, `replace_all`, `extract`, … |
| [docs/builtins/System.md](docs/builtins/System.md) | Référence `System` — `exec`, `env`, `args`, `OS`, `ARCH`, … |
| [docs/builtins/HTTPRequest.md](docs/builtins/HTTPRequest.md) | Référence `HTTPRequest` — `get`, `post`, `send`, `body`, … |
| [examples/README.md](examples/README.md) | Index des exemples de code Ocara |

