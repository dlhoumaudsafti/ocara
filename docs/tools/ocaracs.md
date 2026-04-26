# ocaracs — Analyseur de style Ocara

> Outil de détection de code smell pour les fichiers `.oc`

---

## Installation

```bash
make build-tools
make install-tools   # installe dans /usr/local/bin/ocaracs
```

---

## Usage

```bash
# Analyser un fichier
ocaracs mon_fichier.oc

# Analyser un dossier entier (récursif)
ocaracs examples/

# Suivre les imports automatiquement
ocaracs main.oc      # analyse main.oc + tous ses imports utilisateur
```

`ocaracs` suit les `import` utilisateur (non-builtins) récursivement et déduplique les fichiers déjà analysés.

---

## Format de sortie

Même convention que le compilateur `ocara` (GCC / clang) — chaque ligne est cliquable dans VS Code :

```
fichier.oc:LIGNE:COL: warning: message
```

Exemple :

```
examples/main.oc:5:1: warning: indentation incohérente : espaces attendu(s), tabulations trouvé(s)
examples/main.oc:12:1: warning: ligne vide contient des espaces ou tabulations
examples/main.oc:20:15: warning: espace manquant avant '='
examples/main.oc:33:1: warning: classe 'myPoint' devrait être en PascalCase
```

### Codes de sortie

| Code | Signification |
|------|--------------|
| `0` | Aucun avertissement de style |
| `1` | Avertissement(s) détecté(s) |
| `2` | Erreur d'utilisation |

---

## Configuration

Créer un fichier `.ocaracs` à la racine du projet :

```toml
[rules]
# R01 — cohérence de l'indentation
indentation         = true

# R02 — pas d'espaces ou tabulations sur les lignes vides
empty_lines         = true

# R03 — espaces autour de '=' dans les déclarations var / scoped / const
spacing_assign      = true

# R04 — pas d'espaces ou tabulations en fin de ligne
trailing_whitespace = true

# R05 — longueur max d'une ligne (0 pour désactiver)
max_line_length     = 120

# R06 — max lignes vides consécutives (0 pour désactiver)
blank_lines_max     = 2

# R07 — classes en PascalCase
naming_class        = true

# R08 — fonctions en camelCase (première lettre minuscule)
naming_function     = true

# R09 — constantes en UPPER_SNAKE_CASE
naming_const        = true

# R10 — espace après '//' dans les commentaires
comment_spacing     = true

# R11 — le fichier se termine par une newline
file_ends_newline   = true
```

Si `.ocaracs` est absent, toutes les règles sont activées avec les valeurs par défaut.

---

## Règles

### R01 — Cohérence de l'indentation

La première ligne indentée du fichier détermine l'unité d'indentation globale.  
Toutes les autres lignes indentées doivent utiliser un multiple de cette unité.

```ocara
// Première ligne indentée = 4 espaces → unité = 4
function main(): int {
    var x: int = 1    // ✓ 4 espaces
      var y: int = 2  // ✗ 6 espaces — pas un multiple de 4
	var z: int = 3    // ✗ tabulation — type différent
    return 0
}
```

> Les lignes à l'intérieur d'une chaîne backtick multiligne sont exemptées.

---

### R02 — Lignes vides sans whitespace

Une ligne visiblement vide ne doit contenir aucun espace ni tabulation.

```ocara
function main(): int {
    var x: int = 1
   ← ✗ ligne vide avec 3 espaces
    return x
}
```

> Exemption : contenu d'une chaîne backtick multiligne.

---

### R03 — Espaces autour de `=`

Les déclarations `var`, `scoped` et `const` doivent avoir un espace avant et après `=`.

```ocara
var x: int = 5          // ✓
var x: int=5            // ✗ espace manquant avant et après
var x: int =5           // ✗ espace manquant après
var x: int= 5           // ✗ espace manquant avant

scoped name: string = "hello"   // ✓
const VERSION = "1.0.0"         // ✓
const VERSION="1.0.0"           // ✗
```

Les opérateurs `==`, `!=`, `<=`, `>=`, `=>` sont ignorés.

---

### R04 — Pas d'espaces en fin de ligne

Les lignes non vides ne doivent pas se terminer par des espaces ou tabulations.

---

### R05 — Longueur de ligne

Par défaut, max 120 caractères par ligne. Configurable via `max_line_length`.  
Mettre `max_line_length = 0` pour désactiver.

---

### R06 — Lignes vides consécutives

Par défaut, max 2 lignes vides consécutives. Configurable via `blank_lines_max`.  
Mettre `blank_lines_max = 0` pour désactiver.

---

### R07 — Nommage des classes (PascalCase)

Les classes doivent commencer par une majuscule et n'utiliser que des caractères alphanumériques.

```ocara
class Point { }          // ✓
class httpClient { }     // ✗ → HttpClient
class MY_CLASS { }       // ✗ → MyClass
```

---

### R08 — Nommage des fonctions (camelCase)

Les fonctions doivent commencer par une minuscule.

```ocara
function main(): int { }           // ✓
function calculateArea(): float { } // ✓
function MyFunction(): int { }     // ✗ → myFunction
```

---

### R09 — Nommage des constantes (UPPER_SNAKE_CASE)

Les constantes déclarées avec `const` doivent être en majuscules.

```ocara
const MAX_RETRIES = 3      // ✓
const version = "1.0"     // ✗ → VERSION
const myConst = 42        // ✗ → MY_CONST
```

---

### R10 — Espace après `//`

Les commentaires doivent avoir un espace après `//`.

```ocara
// bon commentaire       ✓
//mauvais commentaire    ✗
///triple slash autorisé ✓  (doc-style)
```

---

### R11 — Newline en fin de fichier

Le fichier doit se terminer par un caractère newline (`\n`).

---

## Intégration Makefile

```bash
make lint           # analyse tous les fichiers de examples/
make build-tools    # compile ocaracs uniquement
make install-tools  # installe ocaracs dans /usr/local/bin/
```

---

## Exemples de configuration `.ocaracs`

### Strict (défaut)

```toml
[rules]
indentation         = true
empty_lines         = true
spacing_assign      = true
trailing_whitespace = true
max_line_length     = 120
blank_lines_max     = 2
naming_class        = true
naming_function     = true
naming_const        = true
comment_spacing     = true
file_ends_newline   = true
```

### Permissif (style libre)

```toml
[rules]
indentation         = true
empty_lines         = true
spacing_assign      = true
trailing_whitespace = false
max_line_length     = 0
blank_lines_max     = 0
naming_class        = false
naming_function     = false
naming_const        = false
comment_spacing     = false
file_ends_newline   = true
```
