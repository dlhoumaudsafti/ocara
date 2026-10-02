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
ocaracs main.oc      # analyse main.oc + tous ses imports utilisateur (hors bibliothèque runtime)
```

`ocaracs` suit les `import` utilisateur (non-runtime) récursivement et déduplique les fichiers déjà analysés.

---

## Correction automatique — `--fix`

```bash
ocaracs --fix main.oc
ocaracs --fix examples/
```

Corrige les fichiers sur place, puis les réanalyse : seuls les avertissements restants sont affichés. **Faites un commit avant** pour pouvoir relire et annuler les changements (`git diff`).

| Règle | Correction |
|---|---|
| R01 | Indentation recalculée d'après les `{ }`, `( )`, `[ ]` (plusieurs ouvrants sur une même ligne ne comptent qu'un niveau), au type et à la largeur de R19/R20 (sinon déduits de la première ligne indentée, sinon 4 espaces). Une ligne de continuation (`.chain()`, opérateur en tête, ou ligne précédente terminée par `=`, `+`, `&&`…) prend un niveau de plus. |
| R02 | Espaces des lignes vides retirés |
| R03 | Espace ajouté avant/après le `=` d'une déclaration |
| R04 | Espaces de fin de ligne retirés |
| R06 | Lignes vides en trop supprimées |
| R10 | Espace ajouté après `//` |
| R11 | Newline ajoutée en fin de fichier |
| R07/R08/R09/R12/R13 | Déclaration renommée au style attendu (`is_adult` → `isAdult`), **avec tous ses usages dans tout le projet** |
| R05 | **Non corrigée** : une ligne trop longue reste à reformuler à la main |

Le contenu des chaînes backtick multilignes n'est jamais modifié.

### Renommage

- **Portée** : tous les `.oc` sous la racine du projet, c'est-à-dire le dossier du `.ocaracs` le plus proche, sinon le dossier analysé. Placez un `.ocaracs` à la racine du projet : sans lui, `ocaracs --fix sous/dossier/x.oc` ne voit pas les fichiers des dossiers parents qui importent `x.oc`.
- **Remplacement** : l'identifiant est remplacé dans le code et dans les `${…}` des backticks, jamais dans une chaîne ni un commentaire. Une méthode, une propriété ou une constante de classe est aussi renommée après `.`/`::` ; une variable ou une fonction ne l'est pas, pour ne jamais toucher un appel de builtin (`s.isEmpty()`).
- **Imports** : seul le dernier segment est renommé (`import lib.car_model` → `import lib.CarModel`), et le fichier `car_model.oc` devient `CarModel.oc`.
- **Renommage ignoré**, et signalé (`ocaracs: 'x' non renommé en 'y' : …`) quand :
  - le nouveau nom est déjà utilisé dans le projet ;
  - le nouveau nom est un mot réservé ;
  - le même nom est attendu sous deux styles différents (une méthode `x` et une constante `x`) ;
  - le nom apparaît dans un fichier non `.oc` du projet (template HTML `${nom}`, script…) ;
  - c'est une méthode de test ocaraunit qui perdrait son suffixe `Test`.

Recompilez après `--fix` : le renommage repose sur les noms, pas sur une analyse sémantique complète.

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

# R07 — classes/structs/interfaces/modules/generics (style : R18)
naming_class        = true

# R08 — fonctions ET méthodes (style : R17)
naming_function     = true

# R09 — constantes globales ET de classe (style : R15)
naming_const        = true

# R10 — espace après '//' dans les commentaires
comment_spacing     = true

# R11 — le fichier se termine par une newline
file_ends_newline   = true

# R12 — variables (var/scoped/consumed) et propriétés (style : R16)
naming_variable     = true

# R13 — const déclarées dans une fonction, une méthode, un bloc runtime ou
# une closure : vérifiées avec leur propre style (R14) ; false = non vérifiées
naming_const_embed  = true

# R14 — style des const locales (par défaut : celui des variables, R16)
# naming_const_embed_is = snake_case

# R15 à R18 — styles : snake_case | camelCase | PascalCase | UPPER_SNAKE_CASE | UPPERCASE
naming_const_is     = UPPER_SNAKE_CASE
naming_var_is       = snake_case
naming_function_is  = camelCase
naming_class_is     = PascalCase

# R19 — type d'indentation : auto (déduit de la 1re ligne indentée) | space | tab
indentation_type    = auto

# R20 — largeur d'un niveau d'indentation (0 = déduite de la 1re ligne indentée)
indentation_gap     = 0
```

Une valeur invalide (`naming_var_is = kebab`) est signalée au chargement et la valeur par défaut est conservée.

> Les valeurs par défaut des règles R07/R08/R09/R12 implémentent [docs/conventions.md](../../docs/conventions.md), la convention de nommage officielle du projet — s'y référer en cas de doute sur une catégorie non couverte ici.

Si `.ocaracs` est absent, toutes les règles sont activées avec les valeurs par défaut.

---

## Règles

### R01 — Cohérence de l'indentation

Par défaut (`indentation_type = auto`, `indentation_gap = 0`), la première ligne indentée du fichier détermine l'unité d'indentation globale.  
Toutes les autres lignes indentées doivent utiliser un multiple de cette unité.
R19 (`indentation_type = space|tab`) impose le type, et R20 (`indentation_gap = N`) impose la largeur d'un niveau : N espaces, ou N tabulations.

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

### R07 — Nommage des classes/structs/interfaces/modules/generics (PascalCase)

`class`, `struct`, `interface`, `module` et `generic` doivent commencer par une majuscule et n'utiliser que des caractères alphanumériques. Style modifiable par R18 (`naming_class_is`).

```ocara
class Point { }              // ✓
class httpClient { }         // ✗ → HttpClient
interface Drawable { }       // ✓
interface loggable { }       // ✗ → Loggable
module Clickable { }         // ✓
generic Stack<T> { }         // ✓
generic cache<K, V> { }      // ✗ → Cache
```

---

### R08 — Nommage des fonctions et méthodes (camelCase)

Les fonctions (`function`) et les méthodes (`method`, avec ou sans visibilité/`static`/`async` devant — une signature de méthode d'interface n'a pas de visibilité) doivent être en camelCase : première lettre minuscule, alphanumérique uniquement, **sans `_`**. Style modifiable par R17 (`naming_function_is`).

```ocara
function main(): int { }              // ✓
function calculateArea(): float { }   // ✓
function MyFunction(): int { }        // ✗ → myFunction
function is_adult(): bool { }         // ✗ → isAdult

public method tryLock(): bool { }         // ✓
public static method Create(): Foo { }    // ✗ → create
method draw(): void                       // ✓ (signature d'interface, pas de visibilité)
```

---

### R09 — Nommage des constantes (UPPER_SNAKE_CASE)

Les constantes globales et les constantes de classe (`public`/`protected`/`private const`) doivent être en majuscules. Style modifiable par R15 (`naming_const_is`). Une `const` déclarée dans un corps relève de R13.

```ocara
const MAX_RETRIES = 3                  // ✓
const version = "1.0"                  // ✗ → VERSION
public const NOT_FOUND:int = 404       // ✓
private const maxRetry:int = 3         // ✗ → MAX_RETRY
```

---

### R13/R14 — Constantes locales

Une `const` déclarée dans un corps est vérifiée avec le style R14 (`naming_const_embed_is`), par défaut celui des variables (R16, `snake_case`). Sont des corps : une fonction, une méthode, un constructeur `init(...)`, une closure `nameless`, un bloc runtime (`init { }`, `main { }`, `error`/`success`/`exit`), ou un fichier runtime entier (`*.runtime.oc`, `*.run.oc`, `*.rt.oc`). Avec `naming_const_embed = false`, ces constantes ne sont plus vérifiées.

```ocara
const MAX_RETRY = 3                       // R09 → UPPER_SNAKE_CASE

function main(): int {
    const db:SQLite = SQLite::open("x.db")   // ✓ R14 → snake_case
    const userCount:int = 0                  // ✗ → user_count
    return 0
}
```

---

### R15 à R18 — Styles de nommage

Valeurs possibles : `snake_case`, `camelCase` (sans `_`), `PascalCase`, `UPPER_SNAKE_CASE`, `UPPERCASE` (sans `_`). Chaque avertissement propose le nom converti (`→ userCount`).

| Règle | Clé | Défaut | Appliquée par |
|---|---|---|---|
| R15 | `naming_const_is` | `UPPER_SNAKE_CASE` | R09 |
| R16 | `naming_var_is` | `snake_case` | R12 (et R14 par défaut) |
| R17 | `naming_function_is` | `camelCase` | R08 |
| R18 | `naming_class_is` | `PascalCase` | R07 |

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

### R12 — Nommage des variables et propriétés (snake_case)

`var`, `scoped`, `consumed` et `property` (avec ou sans visibilité devant pour `property`) doivent être en minuscules avec underscores. Style modifiable par R16 (`naming_var_is`).

```ocara
var user_count:int = 0            // ✓
var userCount:int = 0             // ✗ → user_count
scoped total_price:float = 0.0    // ✓
private property click_count:int  // ✓
public  property FirstName:string // ✗ → first_name
```

> Ne couvre pas les champs NUS d'un `struct` (`id:int`, sans `property`) — reconnaître cette forme demanderait de savoir qu'on est dans le corps d'un struct ; un champ de struct écrit avec `property` reste couvert.
>
> Ne couvre pas les paramètres de fonction/méthode ni les variables de boucle `for`/`for..=>` — `ocaracs` reste un analyseur ligne à ligne, pas un parseur complet ; ces positions demanderaient de suivre une déclaration sur plusieurs lignes ou une syntaxe plus variable que les autres règles.

---

## Intégration Makefile

```bash
make lint-examples  # analyse tous les fichiers de examples/
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
naming_variable     = true
naming_const_embed  = true
indentation_type    = space
indentation_gap     = 4
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
naming_variable     = false
```
