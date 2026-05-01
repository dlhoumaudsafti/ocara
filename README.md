# Ocara

[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/yourusername/ocara/releases)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**O**bject **C**ode **A**bstraction **R**untime **A**rchitecture

Langage de programmation moderne compilé en natif avec typage statique fort et architecture web complète intégrée.  
Serveur HTTP, composants HTML réutilisables avec slots, génération de pages dynamiques — le tout compilé en binaire natif.

> Un langage compilé natif avec composants web intégrés. Performances C++, ergonomie TypeScript, architecture Vue.js — tout dans un binaire zéro-dépendance.

![Logo Ocara](logo.png)

---

## Caractéristiques

- 🛡️ **Typage statique fort** — détection des erreurs à la compilation
- ⚡ **Compilation native** — backend Cranelift pour des performances optimales
- 🌐 **Architecture web intégrée** — serveur HTTP, composants HTML avec slots, routing natif
- 📦 **Bibliothèque standard riche** — HTTP, JSON, Regex, Threads, et plus
- 🎯 **Orienté objet** — classes, interfaces, héritage, méthodes statiques
- ✅ **Tests intégrés** — ocaraunit pour tests unitaires avec couverture
- 🛠️ **Outillage complet** — linter (ocaracs), test runner, diagnostics

---

## Prérequis

| Outil | Version minimale | Rôle |
|---|---|---|
| Rust / Cargo | 1.75+ | Construire le compilateur |
| `cc` (gcc ou clang) | tout | Éditeur de liens final |

### Installer Rust et Cargo

| Plateforme | Installation |
|---|---|
| Debian / Ubuntu | `sudo apt install cargo` |
| Fedora / RHEL | `sudo dnf install cargo` |
| macOS | `brew install rust` ou `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |

### Installer l'éditeur de liens `cc`

`cc` est l'éditeur de liens appelé à chaque compilation d'un fichier `.oc`. Il doit être présent sur le système :

| Plateforme | Installation |
|---|---|
| Debian / Ubuntu | `sudo apt install build-essential` |
| Fedora / RHEL | `sudo dnf install gcc` |
| macOS | `xcode-select --install` |

---

## Démarrage rapide

```bash
# 1. Compiler Ocara
make build

# 2. Créer un fichier hello.oc
cat > hello.oc << 'EOF'
import ocara.IO

function main(): int {
    IO::writeln("Bonjour depuis Ocara !")
    return 0
}
EOF

# 3. Compiler et exécuter
./target/release/ocara hello.oc -o hello
./hello
# Bonjour depuis Ocara !
```

---

## Commandes Makefile

### Compilation

| Commande | Description |
|---|---|
| `make build` | Compile ocara (release, strict) |
| `make build-dev` | Compile ocara (debug, strict) |
| `make build-tools` | Compile les outils (release, strict) |
| `make build-tools-dev` | Compile les outils (debug, strict) |
| `make build-all` | Compile tout (release) |
| `make build-all-dev` | Compile tout (debug) |

### Tests

| Commande | Description |
|---|---|
| `make tests` | Lance les tests unitaires Cargo |
| `make regression` | Teste tous les exemples |
| `make tests-examples` | Lance ocaraunit sur les exemples |
| `make lint-examples` | Lance ocaracs sur les exemples |

### Installation

| Commande | Description |
|---|---|
| `make install` | Installe ocara dans `/usr/local/bin/` |
| `make install-tools` | Installe les outils dans `/usr/local/bin/` |
| `make install-all` | Installe tout (ocara + outils) |

### Désinstallation

| Commande | Description |
|---|---|
| `make uninstall` | Désinstalle ocara |
| `make uninstall-tools` | Désinstalle les outils |
| `make uninstall-all` | Désinstalle tout |

### Nettoyage

| Commande | Description |
|---|---|
| `make clean` | Supprime les artefacts d'ocara |
| `make clean-tools` | Supprime les artefacts des outils |
| `make clean-all` | Supprime tous les artefacts |

---

## Structure du projet

```
Ocara/
├── src/               ← Code source du compilateur (Rust)
├── runtime/           ← Runtime C pour les builtins
├── examples/          ← Exemples de code Ocara (.oc)
├── docs/              ← Documentation complète
└── tools/             ← Outils (ocaraunit, ocaracs)
```

---

## Documentations

| Document | Description |
|---|---|
| [docs/README.md](docs/README.md) | **Index complet de la documentation** — guides, builtins, outils |
| [examples/README.md](examples/README.md) | Index des exemples de code Ocara |
| [tools/ocaraunit/README.md](tools/ocaraunit/README.md) | Runner de tests unitaires avec couverture |
| [tools/ocaracs/README.md](tools/ocaracs/README.md) | Analyseur de style et linter |

---

## Licence

Ocara est distribué sous licence MIT. Voir le fichier [LICENSE](LICENSE) pour plus de détails.

Copyright © 2026 David Lhoumaud

