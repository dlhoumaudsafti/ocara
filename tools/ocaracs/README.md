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

`ocaracs` suit récursivement, comme le compilateur, les fichiers référencés par le fichier d'entrée et déduplique ceux déjà analysés :

- `import a.b.C` et `import X from "a/b"` : résolus depuis le namespace du fichier qui importe (`namespace n` → `n/a/b.oc`), sinon depuis la racine, c'est-à-dire le dossier du fichier d'entrée (`main.oc`) — pas le dossier du fichier qui importe ;
- `import X from "./a"` / `"../a"` : relatifs au fichier qui importe ;
- `runtime a.b` / `runtime a.b is main` : `a/b.runtime.oc`, `.run.oc`, `.rt.oc` puis `.oc` depuis la racine ;
- `wiring a.b.C` (interface, classe de test) : import implicite de `a/b/C.oc` ;
- `import ocara.*` et les modules builtin sont ignorés.

Un import introuvable depuis le fichier d'entrée est signalé (`ocaracs: import introuvable : …`) puis ignoré. Sur un dossier, tous ses `.oc` sont analysés, et leurs imports résolus depuis ce dossier.

Un `.oc` illisible (UTF-8 invalide…) est signalé et fait sortir avec le code 2. Une option inconnue (`--fxi`) est une erreur d'utilisation (code 2) ; `--dry-run` exige `--fix`.

---

## Correction automatique — `--fix`

```bash
ocaracs --fix main.oc
ocaracs --fix examples/
```

Corrige les fichiers sur place, puis les réanalyse : seuls les avertissements restants sont affichés. **Faites un commit avant** pour pouvoir relire et annuler les changements (`git diff`).

`ocaracs --fix --dry-run <cible>` affiche ce que `--fix` ferait (fichiers à modifier, renommages, renommages ignorés), sans rien écrire — c'est ce qu'utilise l'extension VS Code pour demander confirmation avant un renommage.

| Règle | Correction |
|---|---|
| R01 | Indentation recalculée d'après les `{ }`, `( )`, `[ ]` (plusieurs ouvrants sur une même ligne ne comptent qu'un niveau), au type et à la largeur de R19/R20 (sinon déduits de la première ligne indentée, sinon 4 espaces). Une ligne de continuation (`.chain()`, opérateur en tête, ligne précédente terminée par `=`, `+`, `&&`…, ou branche d'une expression conditionnelle hors parenthèses : `then …`, `else …` — pas le `else {` d'un `if` — ou `empty`/`zero`/`null` seul) prend un niveau de plus ; un `*` à la fin d'une chaîne (`` `SELECT *``) ou d'un `import ocara.*` n'est pas un opérateur. |
| R02 | Espaces des lignes vides retirés |
| R03 | Espace ajouté avant/après le `=` d'une déclaration |
| R04 | Espaces de fin de ligne retirés |
| R06 | Lignes vides en trop supprimées |
| R10 | Espace ajouté après `//` |
| R11 | Newline ajoutée en fin de fichier |
| R21 | Lignes vides entre une clause `when` et sa déclaration retirées |
| R22 | Expression `then` à branche trop longue passée à la ligne (`then`/`else` indentés sous la condition), au niveau de l'instruction (`var x = …`, affectation, `return`) ou dans une parenthèse qui ne contient qu'elle ; un ternaire d'une branche découpée est réexaminé. Ailleurs (argument d'appel, condition d'`if`, maillon d'une chaîne, ligne avec un commentaire), l'avertissement reste |
| R07/R08/R09/R12/R13 | Déclaration renommée au style attendu (`is_adult` → `isAdult`), **avec tous ses usages dans tout le projet** |
| R05 | **Non corrigée** : une ligne trop longue reste à reformuler à la main |

Le contenu des chaînes backtick multilignes n'est jamais modifié, et chaque ligne garde sa fin de ligne (un fichier CRLF reste en CRLF ; la newline ajoutée par R11 suit le style du fichier).

### Renommage

- **Portée** : tous les `.oc` sous la racine du projet, c'est-à-dire le dossier du `.ocaracs` le plus proche, sinon le dossier analysé. Placez un `.ocaracs` à la racine du projet : sans lui, `ocaracs --fix sous/dossier/x.oc` ne voit pas les fichiers des dossiers parents qui importent `x.oc`.
- **Remplacement** : l'identifiant est remplacé dans le code et dans les `${…}` des backticks, jamais dans une chaîne ni un commentaire. Une variable ou une fonction n'est jamais renommée après `.`/`::`, pour ne jamais toucher un appel de builtin (`s.isEmpty()`).
- **Membres** (méthode, propriété, constante de classe) : renommés après `.`/`::` seulement quand le receveur est un type du projet — `self`, `parent`, `Classe::`, ou une variable, un paramètre ou une propriété déclaré `nom:Type` avec un type du projet (la déclaration la plus proche qui précède dans le fichier, sinon celles du projet). Sur un receveur builtin (`win.isOpen()` avec `win:SDL`, `IO::`), le membre garde son nom. Si un accès porte sur un receveur de type inconnu (`f().isOpen`, variable de boucle non typée…), le renommage entier est ignoré et signalé.
- **Fichiers et imports** : le dernier segment d'un import désigne à la fois le fichier et le symbole (`import lib.car_model`). Un fichier qui déclare le symbole renommé et porte son nom (`lib/car_model.oc`, classe ou fonction) est renommé avec lui (`lib/CarModel.oc`), et les imports qui le désignent aussi (`import lib.CarModel`, `import X from "./lib/CarModel"`), commentaire de fin compris. Un symbole importé par un fichier qui ne porte pas son nom n'est pas renommé.
- **Renommage ignoré**, et signalé (`ocaracs: 'x' non renommé en 'y' : …`) quand :
  - le nouveau nom est déjà utilisé dans le projet ;
  - deux noms deviendraient le même (`userID` et `userId` → `user_id`) ;
  - c'est un membre accédé sur un receveur de type inconnu ;
  - une variable ou une fonction porte le nom d'un champ nu de struct (`userName:string`), dont les accès `u.userName` ne seraient pas renommés ;
  - le fichier de destination (`CarModel.oc`) existe déjà, ou l'import qui le désigne ne correspond à aucun fichier du même nom ;
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

`COL` est compté en caractères (un `é` compte pour un), comme l'éditeur.

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
| `2` | Erreur d'utilisation (option inconnue, cible introuvable) ou fichier `.oc` illisible |

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

# R21 — clause `when` collée à sa déclaration (aucune ligne vide entre les deux)
when_attached       = true

# R22 — longueur max d'une branche `then`/`else` écrite sur la ligne de sa
# condition (0 pour désactiver)
cond_branch_max_length = 20
```

Une valeur invalide (`naming_var_is = kebab`, `indentation = flase`, `max_line_length = 80x`) est signalée au chargement avec son numéro de ligne (`ocaracs: .ocaracs:3: …`) et la valeur par défaut est conservée ; une clé inconnue ou une ligne sans `=` est signalée et ignorée.

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

### R21 — Clause `when` collée à sa déclaration

Une clause `when` (voir `docs/EBNF.md` §32) se colle à la déclaration ou à l'instruction qu'elle conditionne. Le compilateur accepte des lignes vides entre les deux ; `ocaracs` les signale, et `--fix` les retire. Une ligne qui porte la clause ET sa déclaration (`when default function f() { … }`) n'est pas concernée : la ligne vide qui la suit est conservée.

```ocara
when System::OS is 'linux'        // ✓
function home(): string { return '/home' }

when System::OS is 'windows'      // ✗ ligne vide signalée

function home(): string { return 'C:\\Users' }
```

### R22 — Branche `then`/`else` trop longue

Une expression conditionnelle (`docs/EBNF.md` §11.3) écrite sur une ligne reste lisible tant que ses branches sont courtes. Au-delà de `cond_branch_max_length` caractères (20 par défaut) pour une branche — mesurée sans son mot-clé, chaîne comprise —, `then` et `else` passent à la ligne, indentés sous la condition. Entre parenthèses, la `)` revient seule sur sa ligne. La valeur par défaut de la forme courte (`empty`, `zero`, `null`) n'est pas une branche, et chaque maillon d'une chaîne `x then a else y then b else c` est mesuré à part.

```ocara
var s:string = age greater 17 then "majeur" else "mineur"     // ✓ branches courtes

var f:Function<int(int)> = k greater 2                        // ✓ après --fix
    then nameless(x:int): int { return x + k }
    else nameless(x:int): int { return x }

var palier:int = (age greater 10                              // ✓ après --fix
    then (age greater 30 then 3 else 2)
    else 1
) * 100
```

Le ternaire intérieur reste sur une ligne tant que ses propres branches sont courtes.

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
cond_branch_max_length = 20
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
cond_branch_max_length = 0
```
