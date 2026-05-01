# Ocara — Documentation

> Documentation complète du langage Ocara, de ses builtins et de ses outils.

---

## 📚 Guides généraux

| Document | Description |
|----------|-------------|
| [compilation-guide.md](compilation-guide.md) | Guide complet de compilation avec le compilateur `ocara` |
| [EBNF.md](EBNF.md) | Grammaire formelle du langage Ocara (EBNF) |
| [diagnostics.md](diagnostics.md) | Messages d'erreur et diagnostics du compilateur |

---

## 🔧 Builtins (Classes standard)

Documentation détaillée des classes intégrées au runtime Ocara :

### Entrées/Sorties et Système

| Builtin | Description |
|---------|-------------|
| [IO.md](builtins/IO.md) | Entrées/sorties standard (`writeln`, `read`, `readln`) |
| [File.md](builtins/File.md) | Manipulation de fichiers (`read`, `write`, `exists`, `delete`) |
| [Directory.md](builtins/Directory.md) | Manipulation de répertoires (`create`, `list`, `remove`) |
| [System.md](builtins/System.md) | Informations système (`OS`, `ARCH`, `env`, `exec`, `args`) |

### Manipulation de données

| Builtin | Description |
|---------|-------------|
| [String.md](builtins/String.md) | Manipulation de chaînes (`split`, `trim`, `replace`, `substr`) |
| [Array.md](builtins/Array.md) | Manipulation de tableaux (`push`, `pop`, `sort`, `filter`, `map`) |
| [Map.md](builtins/Map.md) | Manipulation de maps/dictionnaires (`keys`, `values`, `has`, `remove`) |
| [JSON.md](builtins/JSON.md) | Encodage/décodage JSON (`encode`, `decode`, `pretty`, `minimize`) |
| [Regex.md](builtins/Regex.md) | Expressions régulières POSIX ERE (`match`, `replace`, `split`) |
| [Convert.md](builtins/Convert.md) | Conversions entre types (`toInt`, `toFloat`, `toString`, `toBool`) |

### Mathématiques et Dates

| Builtin | Description |
|---------|-------------|
| [Math.md](builtins/Math.md) | Fonctions mathématiques (`sqrt`, `pow`, `sin`, `cos`, `PI`, `E`) |
| [Date.md](builtins/Date.md) | Manipulation de dates (jours/mois/années) |
| [Time.md](builtins/Time.md) | Manipulation d'heures (heures/minutes/secondes) |
| [DateTime.md](builtins/DateTime.md) | Date et heure combinées avec timestamps |

### Réseau et Web

| Builtin | Description |
|---------|-------------|
| [HTTPRequest.md](builtins/HTTPRequest.md) | Requêtes HTTP/HTTPS (`GET`, `POST`, `PUT`, `DELETE`, `PATCH`) |
| [HTTPServer.md](builtins/HTTPServer.md) | Serveur HTTP simple (`listen`, `route`, `response`) |
| [HTML.md](builtins/HTML.md) | Génération de HTML programmatique |
| [HTMLComponent.md](builtins/HTMLComponent.md) | Composants HTML réutilisables |

### Concurrence et Tests

| Builtin | Description |
|---------|-------------|
| [Thread.md](builtins/Thread.md) | Threads natifs (`spawn`, `join`, `sleep`) |
| [Mutex.md](builtins/Mutex.md) | Verrous mutex pour synchronisation (`lock`, `unlock`) |
| [UnitTest.md](builtins/UnitTest.md) | Assertions pour tests unitaires (`assertEquals`, `assertTrue`, `assertFalse`) |

---

## 🛠️ Outils

Documentation des outils de développement Ocara :

| Outil | Documentation | Description |
|-------|---------------|-------------|
| **ocara** | [compilation-guide.md](compilation-guide.md) | Compilateur principal |
| **ocaraunit** | [../tools/ocaraunit/README.md](../tools/ocaraunit/README.md) | Runner de tests unitaires avec couverture |
| **ocaracs** | [../tools/ocaracs/README.md](../tools/ocaracs/README.md) | Analyseur de style et linter |

---

## 📖 Exemples

Voir le dossier [../examples/](../examples/) pour des exemples complets :

- **examples/01-25** : Exemples de base (variables, fonctions, classes, etc.)
- **examples/builtins/** : Démonstrations de chaque classe builtin
- **examples/project/** : Projet multi-fichiers complet avec tests

---

## 📝 Contribuer

Pour ajouter ou modifier la documentation :

1. Les guides généraux vont dans `docs/`
2. Les documentations des builtins vont dans `docs/builtins/`
3. Les documentations des outils vont dans `tools/<outil>/README.md`
4. Mettre à jour ce `README.md` après l'ajout d'un nouveau document
