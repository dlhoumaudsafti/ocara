# Spécification EBNF Ocara v0.1.0

**O**bject **C**ode **A**bstraction **R**untime **A**rchitecture

> Version : **0.1.0**  
> Date : **2026-04-25**  
> Statut : **Officielle**  
> Author : **David Lhoumaud**  

---

## Table des matières

1. [Philosophie](#1-philosophie)
2. [Structure d'un programme](#2-structure-dun-programme)
3. [Système de modules](#3-système-de-modules)
4. [Types](#4-types)
5. [Littéraux et collections](#5-littéraux-et-collections)
6. [Identifiants](#6-identifiants)
7. [Variables et constantes](#7-variables-et-constantes)
8. [Expressions](#8-expressions)
9. [Opérateurs et précédence](#9-opérateurs-et-précédence)
10. [Instructions](#10-instructions)
11. [Blocs](#11-blocs)
12. [Fonctions](#12-fonctions)
13. [Bibliothèque standard runtime](#13-bibliothèque-standard-runtime)
    - [13.1 Classes de la bibliothèque standard runtime (namespace ocara)](#131-classes-de-la-bibliothèque-standard-runtime-namespace-ocara)
14. [Classes](#14-classes)
15. [Interfaces](#15-interfaces)
16. [Héritage et implémentation](#16-héritage-et-implémentation)
17. [Modules (mixins)](#17-modules-mixins)
18. [Enums](#18-enums)
19. [Instanciation](#19-instanciation)
20. [Accès statique](#20-accès-statique)
21. [Conditions](#21-conditions)
22. [Switch](#22-switch)
23. [Match (expression)](#23-match-expression)
24. [Boucles](#24-boucles)
25. [Gestion des erreurs](#25-gestion-des-erreurs)
26. [Résolution des noms](#26-résolution-des-noms)
27. [Grammaire EBNF complète](#27-grammaire-ebnf-complète)

---

## 1. Philosophie

**Ocara** est un langage de programmation **compilé natif**, conçu pour être :

| Propriété                  | Détail                                              |
|----------------------------|-----------------------------------------------------|
| Compilé natif              | Aucune machine virtuelle obligatoire                |
| Fortement typé statique    | Tous les types résolus à la compilation             |
| Orienté objet              | Classes, interfaces, héritage simple                |
| Modulaire                  | Un fichier = un module, imports qualifiés           |
| Simple à parser            | Grammaire non-ambiguë, syntaxe régulière            |
| Sans dépendances runtime   | Pas de GC imposé, pas de runtime externe            |

**Inspirations** :

- Rust : sécurité du typage
- C : compilation native directe
- Java : modèle objet clair
- TypeScript : syntaxe moderne et lisible
- PHP : maps flexibles
- Go programming language : simplicité modules

---

## 2. Structure d'un programme

Un programme Ocara est un ensemble de fichiers sources (extension `.oc`).  
Chaque fichier suit strictement l'ordre suivant :

```
Program ::= ImportDecl*
            ( ConstDecl | EnumDecl | ClassDecl | ModuleDecl | InterfaceDecl | FuncDecl )*
```

**Contraintes d'ordre :**
- Les imports sont toujours en tête de fichier.
- Les déclarations de constantes, enums, classes, modules, interfaces et fonctions peuvent être dans n'importe quel ordre entre elles.
- Il n'existe pas de code de niveau module exécutable hors d'une fonction.

---

## 3. Système de modules

### 3.1 Import

```ebnf
ImportDecl ::= "import" ModulePath ( "as" Identifier )?

ModulePath ::= Identifier ( "." Identifier )*
```

Le chemin de module correspond à un chemin de fichier relatif à la racine du projet :

| Déclaration                   | Fichier correspondant         |
|-------------------------------|-------------------------------|
| `import repository.User`      | `repository/User.oc`          |
| `import datas.User as UserData` | `datas/User.oc`, alias `UserData` |

### 3.2 Règles

- Un alias (`as`) est optionnel.
- Le dernier segment du chemin est le nom du symbole importé.
- Deux imports peuvent pointer vers le même type sous des alias différents.
- Un symbole importé sans alias est accessible par son nom simple **seulement si** aucun symbole local ne le masque.
- Un symbole local masque toujours un import (voir §22).

### 3.3 Exemple

```ocara
import datas.User as UserData
import roles.User as UserAcl
import repository.User
import services.Logger
```

---

## 4. Types

### 4.1 Types primitifs

| Mot-clé  | Description                        |
|----------|------------------------------------|
| `int`    | Entier signé 64 bits               |
| `float`  | Flottant IEEE 754 double précision |
| `string` | Chaîne de caractères UTF-8         |
| `bool`   | Booléen (`true` / `false`)         |
| `mixed`  | Type dynamique, accepte toute valeur — **usage restreint** (voir restrictions ci-dessous) |
| `void`   | Absence de valeur (retour seulement) |

#### Restrictions sur le type `mixed`

Le type `mixed` désactive la vérification de type statique et doit être utilisé **uniquement** dans des contextes spécifiques. Le compilateur applique les règles suivantes :

**❌ Interdictions strictes (erreur de compilation) :**

1. **Interdit comme type de `property`** (champ de classe)
   ```ocara
   class User {
       public property data:mixed  // ❌ ERREUR
   }
   ```
   → Utiliser un type concret ou `map<string, mixed>`

2. **Interdit comme type de retour de fonction/méthode**
   ```ocara
   function get_value(): mixed { }  // ❌ ERREUR
   public method compute(): mixed { }  // ❌ ERREUR
   ```
   → Utiliser des unions explicites : `int|string|null`

**⚠️ Avertissements (warning du compilateur) :**

3. **Variables locales `mixed`** génèrent un warning
   ```ocara
   var temp:mixed = some_value()  // ⚠️ WARNING
   scoped data:mixed = get_data()  // ⚠️ WARNING
   ```
   → Le compilateur suggère d'utiliser un type concret

**✅ Usages autorisés :**

4. **Paramètres de fonctions polymorphes**
   ```ocara
   function log(val:mixed): void {    // ✅ OK
       IO::writeln(val)
   }
   ```

5. **Éléments de collections hétérogènes**
   ```ocara
   var items:mixed[] = [1, "hello", true]              // ✅ OK
   var config:map<string, mixed> = {"port": 8080}      // ✅ OK
   ```

6. **Constantes globales `mixed`** (usage rare)
   ```ocara
   const DEFAULT_VALUE:mixed = null  // ✅ OK (mais déconseillé)
   ```

**Justification :**

Ces restrictions guident vers un typage fort tout en préservant la flexibilité nécessaire pour :
- Les fonctions polymorphes utilitaires (`IO::writeln`, etc.)
- Les structures de données dynamiques (config JSON, etc.)
- L'interopérabilité avec des systèmes externes

> **Recommandation** : privilégier systématiquement les **types union** (`T1|T2|null`) plutôt que `mixed` lorsque les types possibles sont connus à l'avance.

### 4.2 Types composites

```ebnf
Type ::= "int"
       | "float"
       | "string"
       | "bool"
       | "mixed"
       | "void"
       | FunctionType
       | ArrayType
       | MapType
       | QualifiedType
       | UnionType
       | Identifier

FunctionType ::= "Function" "<" Type ">"
ArrayType    ::= Type "[]"
MapType      ::= "map" "<" Type "," Type ">"
QualifiedType ::= Identifier ( "." Identifier )+
UnionType    ::= Type ( "|" Type )+
```

**Exemples :**

```ocara
int
float
string[]
map<string, int>
repository.User
```

### 4.3 Types union

Un type union exprime qu'une valeur peut être de **l'un ou l'autre** des types listés, séparés par `|`.

```ebnf
UnionType ::= Type ( "|" Type )+
```

```ocara
function find(id:int): User|null { ... }
public method parse(raw:string): int|float { ... }
```

**Règles sémantiques :**

- Un union peut combiner n'importe quels types : primitifs, classes, `null`, tableaux, maps.
- `null` dans un union indique une valeur optionnelle (pattern courant : `T|null`).
- La valeur retournée doit être compatible avec **au moins une** variante de l'union.
- L'ordre des variantes est sans importance sémantique.
- Les unions ne sont pas autorisés comme type de `property` — utiliser `mixed` dans ce cas.

```ocara
// OK — retourner null ou un objet
function lookup(key:string): Config|null {
    // ...
    return null
}

// OK — retourner int ou float
function divide(a:int, b:int): int|float {
    if b == 0 { return 0 }
    return a / b
}
```

**Type narrowing (raffinement de type) :**

Ocara v0.1.0 supporte le narrowing via l'opérateur `is` dans les expressions `match` et les conditions, pour **tous les types** :

```ocara
// Narrowing dans match — tous les types supportés
class Animal {
    public property name:string
    init(n:string) { self.name = n }
}

function describe(val:mixed): void {
    match val {
        is null             => IO::writeln("null")
        is int              => IO::writeln("int")
        is float            => IO::writeln("float")
        is string           => IO::writeln("string")
        is int[]            => IO::writeln("array")
        is map<string, int> => IO::writeln("map")
        is Animal           => IO::writeln("object Animal")
        is Function<int>    => IO::writeln("Function")
        default             => IO::writeln("inconnu")
    }
}

// Narrowing dans les conditions
function get_length(s:string|null): int {
    if s is null {
        return 0
    }
    return String::len(s)
}

// Expression is retournant bool
var is_null:bool = val is null
var is_int:bool = val is int
var is_str:bool = val is string
var is_arr:bool = val is int[]
var is_map:bool = val is map<string, int>
var is_obj:bool = val is Animal
var is_fn:bool  = val is Function<int>
```

**Implémentation du type checking runtime :**

Toutes les allocations heap (string, array, map, objet, fat-pointer) sont précédées d'un **header de 8 octets** contenant un tag de type. `is Type` lit ce tag pour une discrimination exacte.

| Opérateur | Mécanisme | Précision |
|-----------|-----------|-----------|
| `is null` | teste `val == 0` | ✓ précis |
| `is int` | teste `val != 0 && val < 65536` | ✓ précis pour les cas usuels |
| `is float` | shortcut statique à la compilation (type connu) | ✓ précis statiquement |
| `is bool` | teste `val == 0 \|\| val == 1` | ⚠️ peut confondre avec int 0 et 1 |
| `is string` | lit le tag header : `TAG_STRING` (1) | ✓ précis |
| `is T[]` | lit le tag header : `TAG_ARRAY` (2) | ✓ précis |
| `is map<K,V>` | lit le tag header : `TAG_MAP` (3) | ✓ précis |
| `is ClassName` | lit le tag header : `TAG_OBJECT` (4) | ✓ précis |
| `is Function<T>` | lit le tag header : `TAG_FUNCTION` (5) | ✓ précis |

**Schéma mémoire avec header :**

```
[tag: i64 — 8 octets]  [données...]
                        ^
                        pointeur retourné au code Ocara
```

**Limitations actuelles (v0.1.0) :**

- `is float` fonctionne uniquement quand le type est connu **statiquement** à la compilation. Dans un contexte `mixed` dynamique, seuls les floats explicitement boxés (via `__box_float`) sont détectables.
- `is bool` peut être confondu avec les `int` 0 et 1.
- `is ClassName` vérifie seulement que la valeur est une instance d'**un** objet (tag `TAG_OBJECT`), sans distinguer les classes entre elles. Pour un narrowing fin par classe, utiliser les patterns dans `on … is ClassName` dans les blocs `try/on`.

### 4.4 Annotation de type

Les variables et paramètres sont obligatoirement annotés :

```ocara
var x:int = 5
scoped name:string = "Alice"
function greet(name:string): void { }
```

### 4.5 Type `Function`

Le type `Function<ReturnType>` représente toute valeur appelable : **fonction libre**, **méthode statique** ou **fonction anonyme** (`nameless`). Les valeurs `Function` sont des *fat pointers* (pointeur de fonction + contexte de capture).

```ebnf
FunctionType ::= "Function" "<" Type ">"
```

```ocara
var f:Function<int> = double                 // référence à une fonction libre
var g:Function<int> = MathOp::square        // référence à une méthode statique
var h:Function<int> = nameless(x:int): int { return x * 2 }  // fonction anonyme
var p:Function<void> = nameless(): void { IO::writeln("tick") }
```

**Règles :**

- `Function<ReturnType>` peut référencer des **fonctions libres**, des **méthodes statiques** et des **fonctions anonymes** (`nameless`).
- L'appel d'une valeur `Function` utilise la syntaxe d'appel normale : `f(args...)` et retourne le type spécifié.
- Le **type de retour** est **obligatoire** et typé statiquement : `Function<int>`, `Function<string|null>`, `Function<void>`, etc.
- Les types de paramètres ne sont **pas** encodés dans le type `Function` — seul le type de retour est vérifié statiquement.
- La compatibilité des types : `Function<T>` est compatible avec `Function<U>` si `T` est compatible avec `U`.
- `Function` n'est pas un mot-clé mais un **type réservé** (PascalCase). Il ne peut pas être utilisé comme nom de classe ou de variable.
- Les fonctions anonymes peuvent capturer des variables locales et `self` depuis leur portée d'enclosement. Toute variable capturée (primitif ou référence) est **promue sur le tas** au moment de la création de la closure : le scope d'origine et la closure partagent la même cellule heap (**shared cell**). Toute mutation — depuis la closure ou depuis le scope extérieur — est immédiatement visible des deux côtés. Voir §12.2 pour les détails.

---

## 5. Littéraux et collections

### 5.1 Littéraux scalaires

```ebnf
Literal  ::= Integer
           | Float
           | String
           | TemplateString
           | Boolean
           | "null"

Integer  ::= Digit+
Float    ::= Digit+ "." Digit+
String   ::= '"' ( EscapeSeq | [^"\n] )* '"'
           | "'" ( EscapeSeq | [^'\n] )* "'"
Boolean  ::= "true" | "false"

EscapeSeq ::= "\" ( "n" | "t" | "r" | '"' | "'" | "\" | "0" )
Digit     ::= [0-9]
```

> **`null`** représente l'absence de valeur pour les types référence (`string`, classes, tableaux, maps).
> Son type inféré est `null`. Il est compatible avec tout type référence mais **pas** avec les types primitifs (`int`, `float`, `bool`).
>
> ```ocara
> var nom:string = null        // OK
> var obj:MonObjet = null      // OK
> var n:int = null             // ERREUR — int n'accepte pas null
> ```

**Séquences d'échappement dans les chaînes :**

| Séquence | Caractère       |
|----------|-----------------|
| `\n`     | Saut de ligne   |
| `\t`     | Tabulation      |
| `\r`     | Retour chariot  |
| `\"`     | Guillemet double |
| `\'`     | Guillemet simple |
| `\\`     | Antislash       |
| `\0`     | Octet nul       |

> **Note :** Les chaînes `"..."` et `'...'` **ne peuvent pas contenir de vraie nouvelle ligne**.
> Utiliser `\n` pour un saut de ligne dans une chaîne simple,
> ou une **chaîne template** pour du texte multiligne réel (voir ci-dessous).

---

### 5.1.1 Chaînes template (backticks)

```ebnf
TemplateString ::= "`" TemplatePart* "`"
TemplatePart   ::= TemplateText | "${" Expression "}"
TemplateText   ::= [^`$] | "$" [^{]
```

Les chaînes délimitées par des backticks `` ` `` offrent deux fonctionnalités :

1. **Interpolation d'expressions** via `${expr}`
2. **Multiligne réel** — les vrais retours à la ligne sont acceptés

```ocara
// Interpolation
scoped nom:string = "David"
IO::writeln(`Bonjour ${nom} !`)   // → Bonjour David !

// Multiligne
IO::write(`Ligne 1
Ligne 2
Ligne 3
`)

// Les deux combinés
IO::write(`Nom  : ${nom}
Age  : ${age}
Ville: ${ville}
`)
```

> Les séquences d'échappement `\n`, `\t`, etc. sont également valides dans les backticks.

### 5.2 Tableaux

```ebnf
ArrayLiteral ::= "[" ( Expression ( "," Expression )* ","? )? "]"
```

Un tableau est une liste ordonnée d'expressions séparées par des virgules.

```ocara
// Tableau simple
var numbers:int[] = [1, 2, 3]

// Tableau multidimensionnel
var matrix:int[][] = [
    [1, 2],
    [3, 4]
]

// Tableau multi-type
var vals:mixed[] = [1, "hello", true]
```

- Le type s'annote `T[]` pour un tableau d'éléments de type `T`.
- Les tableaux multidimensionnels s'écrivent `T[][]`.
- Un tableau `mixed[]` accepte n'importe quel type d'élément.

### 5.3 Tableaux associatifs (map)

```ebnf
MapLiteral ::= "{" MapEntry ( "," MapEntry )* ","? "}"
MapEntry   ::= Expression ":" Expression
```

Un map est une collection de paires clé/valeur. La syntaxe utilise `{ clé: valeur }` — accolades et deux-points.

```ocara
var profile:map<string, mixed> = {
    "name": "Lucas",
    "age":  24
}
```

Les guillemets simples sont acceptes partout :

```ocara
var config:map<string, string> = {
    'lang':  'fr',
    'theme': 'dark'
}
```

- Le type s'annote `map<K, V>` où `K` est le type de clé et `V` le type de valeur.
- Les clés peuvent être de n'importe quel type.
- Un tableau vide s'écrit `[]` et est distinct d'un map vide `{}`.

### 5.4 Accès par index

```ebnf
IndexAccess ::= Expression "[" Expression "]"
```

Valide pour les tableaux et les maps :

```ocara
var first:int   = numbers[0]
var name:string = profile["name"]
```

L'accès par index est un opérateur postfixe (précédence maximale).

---

## 6. Identifiants

```ebnf
Identifier ::= Letter ( Letter | Digit | "_" )*

Letter ::= [a-z] | [A-Z]
Digit  ::= [0-9]
```

**Contraintes :**

- Ne peut pas commencer par un chiffre.
- Sensible à la casse : `User` ≠ `user`.
- Les mots-clés réservés ne peuvent pas être utilisés comme identifiants.

**Mots-clés réservés :**

```
import    as        var        scoped     property   const    function  method
class     interface extends    implements init       static
public    private   protected
if        elseif    else       switch     default    match
while     for       in         return     use        break    continue  self
try       on        is         raise
int       float     string     bool       mixed      map      void
true      false     null
or        and       not
nameless  async     resolve    enum
```

**Types réservés (PascalCase) :**

```
Function
```

> Ces identifiants sont réservés comme types et ne peuvent pas être utilisés comme noms de classe, variable, paramètre ou fonction.

---

## 7. Variables et constantes

### 7.1 Variable (`var`)

```ebnf
VarDecl ::= "var" Identifier ":" Type "=" Expression
```

```ocara
var count:int = 0
count = 42      // réaffectation autorisée
```

`var` déclare une variable **mutable** dont la portée est celle de la fonction. Elle peut être réaffectée à tout moment après sa déclaration.

### 7.2 Variable de bloc (`scoped`)

```ebnf
ScopedDecl ::= "scoped" Identifier ":" Type "=" Expression
```

```ocara
if condition {
    scoped msg:string = "vrai"   // msg existe ici
    IO::writeln(msg)
}                                // msg est libéré ici
```

`scoped` déclare une variable **mutable** dont la portée est strictement limitée au bloc `{ }` courant. Étant mutable, elle peut être réassignée librement dans ce bloc. À la fermeture du bloc, la variable est détruite et sa mémoire libérée.

```ocara
scoped x:int = 1
x = 2   // valide — scoped est mutable
x = x + 10   // valide
```

> **`scoped` est interdit sur un champ de classe** : un champ vit aussi longtemps que l'objet, pas le temps d'un bloc. Utiliser `property` pour les champs de classe.

### 7.3 Constante globale (`const`)

```ebnf
ConstDecl ::= "const" Identifier ":" Type "=" Expression
```

```ocara
const TAX:float = 0.2
const APP_NAME:string = "Ocara"
```

Les constantes globales sont définies **au niveau du module** (hors de toute fonction).  
Leur valeur doit être un littéral ou une expression constante évaluable à la compilation.  
Elles sont accessibles depuis n'importe quelle fonction ou méthode du module.

### 7.4 Constante de classe (`class const`)

```ebnf
ClassConstDecl ::= Visibility "const" Identifier ":" Type "=" Expression
```

```ocara
class Config {
    public const VERSION:string  = "1.0.0"
    public const MAX_RETRY:int   = 3
    protected const TIMEOUT:int  = 30
    private const SECRET:string  = "abc"
}
```

Une constante de classe est une valeur **statique** attachée à la classe (et non à une instance).  
Elle est accessible via l'opérateur `::` sans instanciation :

```ocara
IO::writeln(Config::VERSION)    // "1.0.0"
IO::writeln(Config::MAX_RETRY)  // 3
```

Les règles de visibilité s'appliquent normalement (`public` accessible depuis partout, `protected` depuis la classe et ses sous-classes, `private` depuis la classe uniquement).

---

## 8. Expressions

```ebnf
Expression ::= OrExpr

OrExpr       ::= AndExpr ( "or" AndExpr )*
AndExpr      ::= EqualityExpr ( "and" EqualityExpr )*
EqualityExpr ::= ComparisonExpr ( ( "==" | "!=" ) ComparisonExpr )*
ComparisonExpr ::= RangeExpr ( ( "<" | "<=" | ">" | ">=" ) RangeExpr )*
RangeExpr    ::= AdditiveExpr ( ".." AdditiveExpr )?
AdditiveExpr ::= MultiplicativeExpr ( ( "+" | "-" ) MultiplicativeExpr )*
MultiplicativeExpr ::= UnaryExpr ( ( "*" | "/" | "%" ) UnaryExpr )*
UnaryExpr    ::= ( "not" | "-" ) UnaryExpr
               | PostfixExpr
PostfixExpr  ::= PrimaryExpr PostfixTail*
PostfixTail  ::= "." Identifier ( "(" ArgList? ")" )?
               | "(" ArgList? ")"
               | "[" Expression "]"

PrimaryExpr  ::= Literal
               | "self"
               | NewExpr
               | StaticCall
               | StaticConst
               | MatchExpr
               | NamelessExpr
               | ArrayLiteral
               | MapLiteral
               | "(" Expression ")"
               | Identifier

NewExpr      ::= "use" Identifier "(" ArgList? ")"

NamelessExpr ::= "nameless" "(" ParamList? ")" ( ":" Type )? Block

ArgList ::= Expression ( "," Expression )*
```

### 8.1 Notes importantes

- **Priorité du `..`** : l'opérateur de plage a une précédence inférieure à l'addition — `0..n+1` est `0 .. (n+1)`.
- **Annotation de type postfix** : dans un contexte `match` ou `switch`, l'accès `expr.field:type` est syntaxiquement autorisé ; l'annotation de type est ignorée sémantiquement (hint visuel uniquement).
- **L'appel de fonction** sans receveur est une `PostfixExpr` dont le `PrimaryExpr` est un `Identifier` suivi de `( ArgList? )`.
- **Tableau vs map** : `[...]` est toujours un tableau, `{...}` est toujours un map.

---

## 9. Opérateurs et précédence

Du plus faible au plus fort :

| Niveau | Opérateurs               | Associativité |
|--------|--------------------------|---------------|
| 1      | `or`                     | Gauche        |
| 2      | `and`                    | Gauche        |
| 3      | `==` `!=`                | Gauche        |
| 4      | `<` `<=` `>` `>=`        | Gauche        |
| 5      | `..`                     | Aucune        |
| 6      | `+` `-`                  | Gauche        |
| 7      | `*` `/` `%`              | Gauche        |
| 8      | `not` `-` (unaire)       | Droite        |
| 9      | `.` `()` `[]` (postfix)  | Gauche        |

---

## 10. Instructions

```ebnf
Statement ::= VarDecl
            | ScopedDecl
            | ConstDecl
            | IfStmt
            | SwitchStmt
            | WhileStmt
            | ForStmt
            | ReturnStmt
            | BreakStmt
            | ContinueStmt
            | TryStmt
            | RaiseStmt
            | Expression
```

---

## 11. Blocs

```ebnf
Block ::= "{" Statement* "}"
```

Un bloc ouvre un nouveau scope lexical.  
Les variables déclarées dans un bloc ne sont pas visibles en dehors.

---

## 12. Fonctions

```ebnf
FuncDecl ::= "async"? "function" Identifier "(" ParamList? ")" ":" Type Block

ParamList ::= Param ( "," Param )*
Param     ::= Identifier ":" Type
```

**Règles :**

- Le type de retour est **obligatoire** (utiliser `void` si la fonction ne retourne rien).
- Une fonction `void` peut omettre le `return`.
- Une fonction non-`void` **doit** retourner une valeur sur tous les chemins d'exécution.

**Exemple :**

```ocara
function add(a:int, b:int): int {
    return a + b
}

function greet(name:string): void {
    IO::writeln("Hello " + name)
}
```

### 12.1 Fonctions de première classe

Une fonction peut être passée comme valeur en utilisant le type `Function` (voir §4.5).

```ocara
function double(n:int): int { return n * 2 }

// Passer une fonction libre
function apply(f:Function, n:int): int {
    return f(n)
}
IO::writeln(apply(double, 5))          // 10

// Passer une méthode statique
IO::writeln(apply(MathOp::square, 4)) // 16

// Stocker dans une variable
var op:Function = MathOp::negate
IO::writeln(op(7))                     // -7
```

### 12.2 Fonctions anonymes (`nameless`)

Une **fonction anonyme** est une expression qui produit une valeur de type `Function`. Elle est introduite par le mot-clé `nameless` et peut capturer des variables locales de sa portée d'enclosement (**closure lexicale**).

```ebnf
NamelessExpr ::= "nameless" "(" ParamList? ")" ( ":" Type )? Block
```

**Syntaxe :**

```ocara
var f:Function = nameless(x:int): int {
    return x * 2
}

// Sans paramètre, sans type de retour explicite (void implicite)
var g:Function = nameless(): void {
    IO::writeln("hello")
}
```

**Captures (closures) :**

Une `nameless` capture automatiquement les variables locales et `self` référencés dans son corps mais déclarés dans la portée englobante.

```ocara
var step:int = 5
var inc:Function = nameless(x:int): int {
    return x + step        // `step` est capturé
}
IO::writeln(inc(10))      // 15
```

**Captures de `self` dans une méthode :**

```ocara
class Counter {
    public property value:int
    init(start:int) { self.value = start }

    public method make_adder(step:int): Function {
        return nameless(): void {
            self.value = self.value + step   // `self` et `step` capturés
        }
    }
}
```

**Règles :**

- Le type de retour est **optionnel** ; s'il est omis, `void` est supposé.
- Les captures utilisent une sémantique de **cellule partagée** (shared cell) : au moment de la création de la closure, chaque variable locale capturée est **promue sur le tas** (allocation d'une cellule de 8 octets via `__alloc_obj`). Le scope extérieur et la closure partagent ensuite le même pointeur heap. Toute mutation de la variable — que ce soit depuis la closure ou depuis le scope d'origine — est visible des deux côtés.

| Type capturé | Ce qui est stocké dans l'env | Accès depuis la closure | Mutation visible de l'extérieur ? |
|---|---|---|---|
| `int`, `float`, `bool` | **Pointeur** vers une cellule heap 8 octets | Double-indirection (load du pointeur, puis load de la valeur) | Oui — la cellule est partagée |
| Classe (objet) | **Pointeur** vers le pointeur heap de l'objet | Double-indirection | Oui — le pointeur et l'objet sont partagés |
| Tableau (`T[]`) | **Pointeur** vers le pointeur du tableau | Double-indirection | Oui |
| Map (`map<K,V>`) | **Pointeur** vers le pointeur de la map | Double-indirection | Oui |

```ocara
// Shared cell : mutation extérieure visible dans la closure
var x:int = 10
var f:Function = nameless(): int { return x }
x = 50
IO::writeln(f())    // 50  ← la closure lit la valeur actuelle de x

// Mutations dans la closure persistantes d'un appel à l'autre
var count:int = 0
var inc:Function = nameless(): int { count = count + 1; return count }
inc()   // 1
inc()   // 2
inc()   // 3

// Objet : le pointeur partagé, mutations de champs visibles partout
var user:User = use User("David")
var rename:Function = nameless(): void { user.name = "Bob" }
rename()
IO::writeln(user.name)   // "Bob" — l'objet original est muté
```

- `self` peut être capturé depuis une méthode d'instance ; les mutations de champs via `self` sont visibles depuis l'extérieur.
- Les closures imbriquées ne capturent pas les variables de la closure parente (seulement la portée immédiate).
- Une `nameless` ne peut pas être récursive directement (elle n'a pas de nom).

---

### 12.3 Fonctions asynchrones (`async` / `resolve`)

Une **fonction asynchrone** est déclarée avec le modificateur `async`. Son appel ne bloque pas l'appelant : il retourne immédiatement une **handle de tâche** de type `int`. La valeur finale est récupérée avec l'expression `resolve`.

```ebnf
FuncDecl    ::= "async"? "function" Identifier "(" ParamList? ")" ":" Type Block
ClassMember ::= ... | Visibility "static"? "async"? "method" Identifier "(" ParamList? ")" ":" Type Block
ResolveExpr ::= "resolve" Expression
```

**Syntaxe :**

```ocara
async function compute(n:int): int {
    return n * n
}

function main(): void {
    var t1:int = compute(5)    // lance la tâche, retourne un handle int
    var t2:int = compute(10)
    var r1:int = resolve t1    // attend la fin de t1, retourne le résultat
    var r2:int = resolve t2
}
```

`resolve` peut aussi être utilisé directement sur l'appel :

```ocara
var a:int = resolve compute(6)
```

**Modèle d'exécution :**

| Étape | Mécanique interne |
|-------|------------------|
| Déclaration `async function f(args): T` | Le compilateur génère un wrapper `__async_wrap_f(env: i64): i64` qui dépack les arguments depuis l'env heap et appelle `f`. |
| Appel à `f(...)` (dans un contexte non-`resolve`) | Les arguments sont packagés dans un env heap ; `__task_spawn(wrapper_ptr, env_ptr)` est appelé → crée un thread OS et retourne un `int` (pointeur opaque vers une `OcaraTask`). |
| `resolve expr` | Appel à `__task_resolve(task_ptr)` → joint le thread (`JoinHandle::join`), retourne le résultat sous forme de `int`. |

**Règles :**

- Le type de retour d'une fonction `async` doit être `int` (toute valeur est transmise comme `i64` à travers le thread).
- `resolve` retourne un `int` quel que soit le contexte.
- Une handle ne peut être résolue qu'une seule fois ; une seconde résolution retourne `0`.
- `async` et `nameless` ne peuvent pas être combinés.
- `async` ne modifie pas le type `Function` : une fonction async ne peut pas être passée comme `Function`.

---

## 13. Bibliothèque standard runtime

> **Déprécié.** Les alias globaux `write` et `read` sont conservés pour la compatibilité ascendante mais ne doivent plus être utilisés dans le code nouveau.
> Utiliser à la place `IO::writeln` et `IO::read` du module `ocara.IO`.

```ocara
import ocara.IO

IO::writeln("Bonjour !")     // canonical
IO::writeln(42)
IO::writeln(true)

IO::writeln("Quel est ton nom ?")
scoped nom:string = IO::read()   // lecture clavier
IO::writeln("Bonjour " + nom)
```

| Alias déprécié | Équivalent canonique      | Description                                        |
|----------------|---------------------------|----------------------------------------------------|
| `write(val)`   | `IO::writeln(val:mixed)`  | Affiche une valeur sur la sortie standard (stdout) |
| `read()`       | `IO::read(): string`      | Lit une ligne saisie au clavier (stdin)            |

**Notes :**

- `IO::writeln` accepte n'importe quel type (`mixed`) : entier, flottant, booléen, chaîne, tableau, etc.
- `IO::read` retourne toujours une valeur de type `string`.
- Le module `ocara.IO` doit être importé explicitement : `import ocara.IO`.

### 13.1 Classes de la bibliothèque standard runtime (namespace ocara)

Le runtime Ocara fournit un ensemble de classes prédéfinies dans le namespace `ocara.*`. Ces classes sont compilées dans le runtime et disponibles via import explicite (`import ocara.Classe`).

#### Entrées/Sorties

- **IO** — Lecture/écriture console (stdin/stdout/stderr)
  - `writeln()`, `write()`, `read()`, `write_stderr()`, `flush()`
- **HTTPRequest** — Client HTTP pour requêtes GET/POST
  - `get()`, `post()`, `set_header()`, `get_status()`, `get_body()`
- **HTTPServer** — Serveur HTTP embarqué
  - `start()`, `stop()`, `handle()`, `send_response()`

#### Manipulation de données

- **Array** — Opérations sur les tableaux
  - `push()`, `pop()`, `shift()`, `unshift()`, `slice()`, `join()`, `length()`, `sort()`, `reverse()`, `contains()`, `index_of()`, `remove()`
- **Map** — Opérations sur les dictionnaires (clé-valeur)
  - `set()`, `get()`, `has()`, `remove()`, `keys()`, `values()`, `size()`, `clear()`, `merge()`
- **String** — Manipulation de chaînes
  - `length()`, `substring()`, `index_of()`, `split()`, `replace()`, `to_upper()`, `to_lower()`, `trim()`, `starts_with()`, `ends_with()`, `char_at()`
- **Regex** — Expressions régulières PCRE
  - `match()`, `test()`, `replace()`, `split()`, `match_all()`

#### Utilitaires

- **Math** — Fonctions mathématiques (classe statique)
  - `abs()`, `sqrt()`, `pow()`, `sin()`, `cos()`, `tan()`, `floor()`, `ceil()`, `round()`, `min()`, `max()`, `random()`, `PI`, `E`
- **Convert** — Conversions de types (classe statique)
  - `int_to_str()`, `str_to_int()`, `float_to_str()`, `str_to_float()`, `bool_to_str()`, `char_to_int()`, `int_to_char()`
- **System** — Informations système et exécution de commandes (classe statique)
  - `os()`, `arch()`, `exec()`, `exit()`, `env()`, `args()`

#### Date et Heure

- **DateTime** — Manipulation de timestamps Unix (classe statique)
  - `now()`, `from_timestamp()`, `year()`, `month()`, `day()`, `hour()`, `minute()`, `second()`, `format()`, `parse()`
- **Date** — Manipulation de dates sans heure (classe statique)
  - `today()`, `from_timestamp()`, `year()`, `month()`, `day()`, `day_of_week()`, `is_leap_year()`, `days_in_month()`, `add_days()`, `diff_days()`
- **Time** — Manipulation d'heures sans date (classe statique)
  - `now()`, `from_timestamp()`, `hour()`, `minute()`, `second()`, `from_seconds()`, `to_seconds()`, `add_seconds()`, `diff_seconds()`

#### Concurrence

- **Thread** — Gestion de threads natifs (classe d'instance)
  - `run()`, `join()`, `detach()`, `id()`, `sleep()`, `current_id()`
- **Mutex** — Synchronisation thread-safe (classe d'instance)
  - `lock()`, `unlock()`, `try_lock()`

#### Tests

- **UnitTest** — Assertions pour tests unitaires (classe statique)
  - `assertEquals()`, `assertNotEquals()`, `assertTrue()`, `assertFalse()`, `assertNull()`, `assertNotNull()`, `assertGreater()`, `assertLess()`

**Utilisation :**

```ocara
import ocara.DateTime
import ocara.IO
import ocara.Convert

function main(): void {
    var now:int = DateTime::now()
    var date:string = DateTime::from_timestamp(now)
    IO::writeln("Date actuelle : " + date)
    
    var year:int = DateTime::year(now)
    IO::writeln("Année : " + Convert::int_to_str(year))
}
```

**Documentation détaillée :** Voir `docs/builtins/` pour la documentation complète de chaque classe de la bibliothèque standard runtime.

---

## 14. Classes

```ebnf
ClassDecl  ::= "class" Identifier
               ( "extends" Identifier )?
               ( "modules" Identifier ( "," Identifier )* )?
               ( "implements" Identifier ( "," Identifier )* )?
               ClassBody

ClassBody  ::= "{" ClassMember* "}"

ClassMember ::= Constructor
              | Visibility "static"? "method" Identifier "(" ParamList? ")" ":" Type Block
              | Visibility "property" Identifier ":" Type
              | Visibility "const" Identifier ":" Type "=" Expression

Constructor ::= "init" "(" ParamList? ")" Block

Visibility  ::= "public" | "private" | "protected"
```

### 14.1 Composition avec modules (mixins)

Les modules permettent de composer des comportements réutilisables dans une classe via le mot-clé `modules` :

```ocara
class User modules Timestamped, Identifiable {
    // ... membres de la classe
}
```

Les champs et méthodes des modules sont ajoutés à la classe comme si ils avaient été déclarés directement dans celle-ci. Si une classe définit une méthode avec le même nom qu'une méthode d'un module, la méthode de la classe prend la priorité (surcharge).

**Règles :**
- Les modules sont appliqués dans l'ordre de déclaration
- Les champs des modules sont ajoutés avant les champs de la classe
- Les méthodes des modules non surchargées sont disponibles sur les instances de la classe
- Les modules ne peuvent pas avoir de constructeurs

**Voir aussi :** Section 28 — Modules (mixins)

### 14.2 Constructeur (`init`)

Le constructeur est déclaré avec le mot-clé `init`. Il est **toujours public** : aucun mot-clé de visibilité ne peut le précéder. Écrire `public init(...)` est une erreur de syntaxe.

```ebnf
Constructor ::= "init" "(" ParamList? ")" Block
```

> La règle EBNF ne comporte pas de `Visibility` en préfixe : c'est intentionnel.
> La visibilité publique est implicite et non configurable.

**Règles :**

- Nommé `init`, sans type de retour.
- Au plus un constructeur par classe.
- Appel via `use ClassName(args)`.
- Ne peut pas être `private`, `protected` ou `static`.

### 14.2 Membres

| Visibilité  | Accès                                    |
|-------------|------------------------------------------|
| `public`    | Depuis n'importe quel contexte           |
| `private`   | Depuis la classe courante uniquement     |
| `protected` | Depuis la classe et ses sous-classes     |

- `property` : champ d'instance d'une classe — **obligatoire** pour les champs. `var` et `scoped` sont **interdits** sur un champ de classe.
- `const` : constante **statique** de classe, accessible via `Class::NAME`

> **Initialisation implicite des `property`** : tout champ non assigné dans `init` est automatiquement mis à zéro par le runtime (`alloc_zeroed`).
> - Type référence (`string`, classe, tableau, map) → `null` (pointeur nul)
> - Type primitif (`int`, `float`) → `0`
> - `bool` → `false`
>
> ```ocara
> class Personne {
>     public property nom:string   // → null si non assigné dans init
>     public property age:int      // → 0   si non assigné dans init
>
>     init() { }   // rien assigné
> }
>
> var p:Personne = use Personne()
> IO::writeln(p.nom)   // null
> IO::writeln(p.age)   // 0
> ```
>
> Ce comportement est garanti mais **implicite** : préférer une initialisation explicite dans `init` pour que l'intention soit claire.
> Contrairement à `var` (qui oblige une valeur à la déclaration), une `property` ne requiert pas de valeur dans la déclaration.

### 14.3 Constantes de classe

```ocara
class Config {
    public const VERSION:string = "1.0.0"
    public const MAX_RETRY:int  = 3
    protected const TIMEOUT:int = 30
}
```

Les constantes de classe sont accessibles sans créer d'instance, via `::` :

```ocara
IO::writeln(Config::VERSION)    // "1.0.0"
IO::writeln(Config::MAX_RETRY)  // 3
```

Elles ne peuvent pas être modifiées. Les règles de visibilité s'appliquent normalement.

### 14.4 Méthodes statiques

Une méthode préfixée par `static` appartient à la classe et non à une instance. Elle s'appelle via `::` sans créer d'objet.

```ocara
class MathUtils {
    public static method pow(base:int, exp:int): int {
        var result:int = 1
        for i in 0..exp {
            result = result * base
        }
        return result
    }
}

var r:int = MathUtils::pow(2, 8)   // 256
```

#### Appel inter-statique avec `self::`

Depuis l'intérieur d'une classe, une méthode statique peut en appeler une autre de la **même classe** avec `self::` sans répéter le nom de la classe. C'est un raccourci pour `ClassName::method()`.

```ocara
class Validator {
    public static method is_positive(n:int): bool {
        return n > 0
    }

    public static method are_both_positive(a:int, b:int): bool {
        return self::is_positive(a) and self::is_positive(b)
    }
}
```

> **Règles :**
> - `self::method()` n'est valide qu'à l'intérieur d'une méthode ou du constructeur d'une classe.
> - `self::` appelle uniquement des méthodes `static` — pas des méthodes d'instance.
> - Depuis l'extérieur de la classe, on utilise toujours `ClassName::method()`.

### 14.5 `self`

Le mot-clé `self` référence l'instance courante à l'intérieur des méthodes et du constructeur.

```ocara
class User {
    private property name:string

    init(name:string) {
        self.name = name
    }

    public method greet(): void {
        IO::writeln("Hello " + self.name)
    }
}
```

---

## 15. Interfaces

```ebnf
InterfaceDecl ::= "interface" Identifier "{" InterfaceMethod* "}"

InterfaceMethod ::= "method" Identifier "(" ParamList? ")" ":" Type
```

- Une interface déclare uniquement des signatures de méthodes (pas de corps).
- Pas de champs dans une interface.
- Une classe implémentant une interface doit fournir toutes ses méthodes.

```ocara
interface Logger {
    method log(msg:string): void
    method error(msg:string): void
}
```

---

## 16. Héritage et implémentation

```ebnf
Inheritance  ::= "extends" Identifier
Interfaces   ::= "implements" Identifier ( "," Identifier )*
```

**Règles :**

- L'héritage est **simple** (une seule classe parente).
- Une classe peut implémenter **plusieurs** interfaces.
- `extends` et `implements` sont indépendants et optionnels.

```ocara
class ConsoleLogger implements Logger {
    public method log(msg:string): void {
        IO::writeln(msg)
    }
}

class AdminLogger extends ConsoleLogger implements Logger, Auditable {
    public method log(msg:string): void {
        IO::writeln("[ADMIN] " + msg)
    }
}
```

---

## 17. Modules (mixins)

Les **modules** (ou **mixins**) permettent la composition horizontale de comportements réutilisables. Un module est similaire à une classe, mais il ne peut pas être instancié directement. Ses membres (champs, méthodes, constantes) sont incorporés dans les classes qui l'utilisent via le mot-clé `modules`.

```ebnf
ModuleDecl ::= "module" Identifier ClassBody

ClassDecl  ::= "class" Identifier
               ( "extends" Identifier )?
               ( "modules" Identifier ( "," Identifier )* )?
               ( "implements" Identifier ( "," Identifier )* )?
               ClassBody
```

### 17.1 Déclaration d'un module

```ocara
module Timestamped {
    private property created_at: int

    public method mark_created(): void {
        self.created_at = System::time()
    }

    public method get_age(): int {
        return System::time() - self.created_at
    }
}
```

### 17.2 Utilisation dans une classe

```ocara
class User modules Timestamped {
    private property name: string

    init(n: string) {
        self.name = n
        self.created_at = 0  // champ du module
    }

    public method get_name(): string {
        return self.name
    }
}

function main(): int {
    var u: User = use User("Alice")
    u.mark_created()          // méthode du module
    IO::writeln(u.get_age())  // méthode du module
    return 0
}
```

### 17.3 Règles de composition

- **Ordre des modules** : les modules sont appliqués dans l'ordre de déclaration (`modules A, B` → A puis B)
- **Champs** : les champs des modules sont ajoutés avant les champs de la classe
- **Surcharge** : si une classe définit une méthode avec le même nom qu'une méthode d'un module, la méthode de la classe prend la priorité
- **Constructeur** : les modules ne peuvent pas avoir de constructeur `init`. Le constructeur de la classe doit initialiser les champs des modules
- **Visibilité** : les règles de visibilité (`public`, `private`, `protected`) s'appliquent normalement
- **Multiple composition** : une classe peut utiliser plusieurs modules

### 17.4 Conflits de noms

Si deux modules définissent une méthode ou un champ avec le même nom, le dernier module déclaré prend la priorité. Si la classe elle-même définit un membre avec le même nom, la classe l'emporte.

```ocara
module A {
    public method greet(): string {
        return "Hello from A"
    }
}

module B {
    public method greet(): string {
        return "Hello from B"
    }
}

class C modules A, B {
    // B.greet() prend la priorité car B est après A
}

class D modules A, B {
    public method greet(): string {
        return "Hello from D"  // D.greet() prend la priorité sur A et B
    }
}
```

---

## 18. Enums

```ebnf
EnumDecl    ::= "enum" Identifier "{" EnumVariant ( "," EnumVariant )* ","? "}"
EnumVariant ::= Identifier ( "=" Integer )?
```

Un enum définit un ensemble de variantes nommées qui compilent vers des constantes entières. Les variantes sont accessibles via l'opérateur `::` sans instanciation.

```ocara
// Valeurs automatiques : 0, 1, 2, 3
enum Direction {
    North,
    East,
    South,
    West
}

// Valeurs explicites
enum HttpStatus {
    Ok      = 200,
    Created = 201,
    NotFound = 404,
    Error   = 500
}

var d:int = Direction::North    // 0
var s:int = HttpStatus::NotFound // 404
```

**Règles :**

- Les variantes sans valeur explicite sont numérotées automatiquement à partir de 0 (ou à partir de la valeur précédente + 1).
- Les variantes avec valeur explicite doivent être des **entiers littéraux** (`int`).
- Une variante d'enum a le type `int` — elle peut être utilisée partout où un `int` est attendu.
- Les variantes sont accessibles via `EnumName::VariantName` (syntaxe `StaticConst`).
- `enum` n'est pas instanciable via `use`.
- Les noms de variantes doivent être uniques dans leur enum.
- La virgule finale est optionnelle.

---

## 19. Instanciation

```ebnf
NewExpr ::= "use" Identifier "(" ArgList? ")"
```

Le mot-clé `use` appelle le constructeur `init` de la classe.

```ocara
var user:User = use User("David", 43)
var logger:Logger = use ConsoleLogger()
```

---

## 20. Accès statique

```ebnf
StaticCallee ::= Identifier | "self"
StaticCall   ::= StaticCallee "::" Identifier "(" ArgList? ")"
StaticConst  ::= StaticCallee "::" Identifier
```

Appel d'une méthode statique ou lecture d'une constante de classe, sans instanciation. `self::` est utilisable uniquement depuis l'intérieur d'une classe pour référencer la classe courante.

`StaticConst` sans `()` produit une **référence de fonction** (`Function`) lorsque le membre désigné est une méthode statique.

```ocara
var result:int = Math::abs(-5)         // appel statique
var s:string = String::from(42)

var f:Function = MathOp::square        // référence — pas d'appel
var g:Function = self::is_positive     // référence depuis l'intérieur

class Validator {
    public static method is_positive(n:int): bool { return n > 0 }

    public static method are_both_positive(a:int, b:int): bool {
        return self::is_positive(a) and self::is_positive(b)
    }
}
```

---

## 21. Conditions

```ebnf
IfStmt ::= "if" Expression Block
           ( "elseif" Expression Block )*
           ( "else" Block )?
```

```ocara
if x > 0 {
    IO::writeln("positif")
} elseif x == 0 {
    IO::writeln("zéro")
} else {
    IO::writeln("négatif")
}
```

---

## 22. Switch

```ebnf
SwitchStmt  ::= "switch" Expression "{" SwitchCase* DefaultCase? "}"

SwitchCase  ::= Literal Block
DefaultCase ::= "default" Block
```

- Chaque cas est un littéral (entier, flottant, chaîne, booléen).
- Il n'y a **pas** de `break` : chaque cas est isolé.
- `default` est optionnel.

```ocara
switch code {
    200 {
        IO::writeln("OK")
    }
    404 {
        IO::writeln("Not Found")
    }
    default {
        IO::writeln("Unknown")
    }
}
```

---

## 23. Match (expression)

```ebnf
MatchExpr ::= "match" PostfixExpr "{" MatchArm+ "}"

MatchArm ::= MatchPattern "=>" Expression
           | "default" "=>" Expression

MatchPattern ::= Literal
               | "is" Type
```

`match` est une **expression** (retourne une valeur). Chaque bras produit une valeur.

```ocara
scoped label:string = match score {
    100 => "parfait"
    90  => "excellent"
    default => "insuffisant"
}
```

**Patterns de type avec `is` :**

```ocara
function process(val:int|string|null): void {
    match val {
        is null   => IO::writeln("Valeur nulle")
        is int    => IO::writeln("Entier")
        is string => IO::writeln("Chaîne")
    }
}
```

**Mélange de patterns :**

Les patterns littéraux et les patterns de type peuvent être mélangés dans un même `match` :

```ocara
match x {
    is null => IO::writeln("null")
    42      => IO::writeln("quarante-deux")
    default => IO::writeln("autre")
}
```

L'annotation de type postfix est autorisée sur le sujet :

```ocara
scoped desc:string = match user.age:int {
    43 => "vieux"
    20 => "jeune"
    default => "inconnu"
}
```

---

## 24. Boucles

### 22.1 While

```ebnf
WhileStmt ::= "while" Expression Block
```

```ocara
while x > 0 {
    x = x - 1
}
```

### 22.2 For (itération simple)

```ebnf
ForInStmt ::= "for" Identifier "in" Expression Block
```

```ocara
for i in 0..5 {
    IO::writeln(i)
}
```

### 22.3 For (paires clé/valeur)

```ebnf
ForMapStmt ::= "for" Identifier "=>" Identifier "in" Expression Block
```

```ocara
for key => value in profile {
    IO::writeln(key + " = " + value)
}
```

### 22.4 Opérateur de plage

```ebnf
RangeExpr ::= AdditiveExpr ".." AdditiveExpr
```

Produit une séquence d'entiers de `start` inclus à `end` **exclus**.

```ocara
0..5    // 0, 1, 2, 3, 4
1..n+1  // 1, 2, …, n
```

### 22.5 Break

```ebnf
BreakStmt ::= "break"
```

Interrompt immédiatement la boucle courante (`while`, `for..in`, `for..range`). L'exécution reprend à l'instruction suivant la boucle.

```ocara
var i:int = 0
while i < 10 {
    if i == 5 {
        break
    }
    i = i + 1
}
// i vaut 5 ici
```

> `break` n'est valide qu'à l'intérieur d'une boucle. En dehors, c'est une erreur de compilation.

### 22.6 Continue

```ebnf
ContinueStmt ::= "continue"
```

Passe immédiatement à l'**itération suivante** de la boucle courante. Pour un `for..range` ou `for..in`, l'incrément est exécuté avant de réévaluer la condition.

```ocara
for i in 0..10 {
    if i % 2 == 0 {
        continue
    }
    IO::writeln(i)   // affiche uniquement les impairs
}
```

> `continue` n'est valide qu'à l'intérieur d'une boucle.

---

## 25. Gestion des erreurs

```ebnf
TryStmt  ::= "try" Block OnClause+
OnClause ::= "on" Identifier ( "is" Identifier )? Block

RaiseStmt ::= "raise" Expression
```

### 23.1 `try` / `on`

Le bloc `try` exécute du code susceptible de lever une erreur. Chaque clause `on` définit un handler avec un **binding explicite** — le nom après `on` est la variable qui contiendra l'erreur capturée.

```ocara
try {
    var data:string = IO::read()
} on e {
    raise `erreur inattendue : ${e}`
}
```

### 23.2 Filtrage par classe (`is`)

La variante `on <binding> is <Classe>` filtre les erreurs par type. Plusieurs handlers peuvent être chaînés, du plus spécifique au plus général. Le premier handler dont le type correspond est exécuté.

```ocara
try {
    var data:string = IO::read()
} on e is IOException {
    raise `IO : ${e.message}`
} on e is NetworkError {
    raise `réseau : ${e.message}`
} on e {
    raise `inconnu : ${e}`
}
```

> Le handler générique (`on e` sans `is`) doit toujours être placé en dernier.

### 23.3 `raise`

`raise` lève une erreur. Il accepte n'importe quelle expression : chaîne, template string, ou instance d'une classe d'exception.

```ocara
raise "quelque chose a mal tourné"
raise `code erreur : ${code}`
raise use IOException("Fichier introuvable", 404)
```

> `raise` interrompt immédiatement l'exécution du bloc courant. En dehors d'un `on`, l'erreur remonte la pile d'appels.

### 23.4 Classe d'exception

Une exception est une **classe ordinaire** — aucune interface ni classe de base requise. Par convention, les classes d'exception ont un champ `message:string`.

```ocara
class IOException {
    public property message: string
    public property code: int

    init(message: string, code: int) {
        self.message = message
        self.code    = code
    }
}

try {
    raise use IOException("Fichier introuvable", 404)
} on e is IOException {
    IO::writeln(`[${e.code}] ${e.message}`)
}
```

---

## 26. Résolution des noms

L'ordre de résolution strict est le suivant (priorité décroissante) :

| Priorité | Portée                         |
|----------|--------------------------------|
| 1        | Variables locales du bloc courant |
| 2        | Paramètres de la fonction      |
| 3        | Membres de la classe courante  |
| 4        | Classes et types déclarés localement |
| 5        | Symboles importés              |

**Règle fondamentale** : un symbole local **masque toujours** un import.  
Un import ne peut jamais écraser un symbole local existant.

---

## 27. Grammaire EBNF complète

> Notation : `*` = zéro ou plus, `+` = un ou plus, `?` = optionnel, `|` = alternative, `( )` = groupement.

```ebnf
(* ── Programme ─────────────────────────────────────────────────── *)

Program     ::= ImportDecl*
                ( ConstDecl | EnumDecl | ClassDecl | ModuleDecl | InterfaceDecl | FuncDecl )*

(* ── Imports ────────────────────────────────────────────────────── *)

ImportDecl  ::= "import" ModulePath ( "as" Identifier )?
ModulePath  ::= Identifier ( "." Identifier )*

(* ── Déclarations globales ──────────────────────────────────────── *)

ConstDecl   ::= "const" Identifier ":" Type "=" Expression
EnumDecl    ::= "enum" Identifier "{" EnumVariant ( "," EnumVariant )* ","? "}"
EnumVariant ::= Identifier ( "=" Integer )?
ClassDecl   ::= "class" Identifier
                ( "extends" Identifier )?
                ( "modules" Identifier ( "," Identifier )* )?
                ( "implements" Identifier ( "," Identifier )* )?
                ClassBody
InterfaceDecl ::= "interface" Identifier "{" InterfaceMethod* "}"
FuncDecl    ::= "async"? "function" Identifier "(" ParamList? ")" ":" Type Block

(* ── Classe ─────────────────────────────────────────────────────── *)

ClassBody   ::= "{" ClassMember* "}"
ClassMember ::= Constructor
              | Visibility "static"? "async"? "method" Identifier "(" ParamList? ")" ":" Type Block
              | Visibility "property" Identifier ":" Type
              | Visibility "const" Identifier ":" Type "=" Expression
Constructor ::= "init" "(" ParamList? ")" Block
Visibility  ::= "public" | "private" | "protected"

(* ── Interface ──────────────────────────────────────────────────── *)

InterfaceMethod ::= "method" Identifier "(" ParamList? ")" ":" Type

(* ── Paramètres ─────────────────────────────────────────────────── *)

ParamList   ::= Param ( "," Param )*
Param       ::= Identifier ":" Type

(* ── Types ──────────────────────────────────────────────────────── *)

Type        ::= "int" | "float" | "string" | "bool" | "mixed" | "void"
              | FunctionType
              | ArrayType
              | MapType
              | QualifiedType
              | UnionType
              | Identifier
FunctionType  ::= "Function" "<" Type ">"
ArrayType   ::= Type "[]"
MapType     ::= "map" "<" Type "," Type ">"
QualifiedType ::= Identifier ( "." Identifier )+
UnionType   ::= Type ( "|" Type )+

(* ── Bloc et instructions ───────────────────────────────────────── *)

Block       ::= "{" Statement* "}"

Statement   ::= VarDecl
              | ScopedDecl
              | ConstDecl
              | IfStmt
              | SwitchStmt
              | WhileStmt
              | ForStmt
              | ReturnStmt
              | BreakStmt
              | ContinueStmt
              | TryStmt
              | RaiseStmt
              | Expression

VarDecl     ::= "var" Identifier ":" Type "=" Expression
ScopedDecl  ::= "scoped" Identifier ":" Type "=" Expression
ReturnStmt  ::= "return" Expression?
BreakStmt    ::= "break"
ContinueStmt ::= "continue"
TryStmt      ::= "try" Block OnClause+
OnClause     ::= "on" Identifier ( "is" Identifier )? Block
RaiseStmt     ::= "raise" Expression

(* ── Conditions ─────────────────────────────────────────────────── *)

IfStmt      ::= "if" Expression Block
                ( "elseif" Expression Block )*
                ( "else" Block )?

(* ── Switch ─────────────────────────────────────────────────────── *)

SwitchStmt  ::= "switch" Expression "{" SwitchCase* ( "default" Block )? "}"
SwitchCase  ::= Literal Block

(* ── Boucles ────────────────────────────────────────────────────── *)

ForStmt     ::= "for" Identifier "in" Expression Block
              | "for" Identifier "=>" Identifier "in" Expression Block
WhileStmt   ::= "while" Expression Block

(* ── Expressions (hiérarchie de précédence) ─────────────────────── *)

Expression  ::= OrExpr
OrExpr      ::= AndExpr ( "or" AndExpr )*
AndExpr     ::= EqualityExpr ( "and" EqualityExpr )*
EqualityExpr ::= ComparisonExpr ( ( "==" | "!=" ) ComparisonExpr )*
ComparisonExpr ::= RangeExpr ( ( "<" | "<=" | ">" | ">=" ) RangeExpr )*
RangeExpr   ::= AdditiveExpr ( ".." AdditiveExpr )?
AdditiveExpr ::= MultiplicativeExpr ( ( "+" | "-" ) MultiplicativeExpr )*
MultiplicativeExpr ::= UnaryExpr ( ( "*" | "/" | "%" ) UnaryExpr )*
UnaryExpr   ::= ( "not" | "-" | "resolve" ) UnaryExpr | PostfixExpr
PostfixExpr ::= PrimaryExpr PostfixTail*
PostfixTail ::= "." Identifier ( "(" ArgList? ")" )?
              | "(" ArgList? ")"
              | "[" Expression "]"

PrimaryExpr ::= Literal
              | "self"
              | NewExpr
              | StaticCall
              | StaticConst
              | MatchExpr
              | NamelessExpr
              | ArrayLiteral
              | MapLiteral
              | "(" Expression ")"
              | Identifier

NewExpr      ::= "use" Identifier "(" ArgList? ")"
NamelessExpr ::= "nameless" "(" ParamList? ")" ( ":" Type )? Block
StaticCallee ::= Identifier | "self"
StaticCall  ::= StaticCallee "::" Identifier "(" ArgList? ")"
StaticConst ::= StaticCallee "::" Identifier
ArrayLiteral ::= "[" ( Expression ( "," Expression )* ","? )? "]"
MapLiteral   ::= "{" MapEntry ( "," MapEntry )* ","? "}"
MapEntry     ::= Expression ":" Expression
ArgList     ::= Expression ( "," Expression )*

(* ── Match expression ───────────────────────────────────────────── *)

MatchExpr    ::= "match" PostfixExpr "{" MatchArm+ "}"
MatchArm     ::= MatchPattern "=>" Expression
               | "default" "=>" Expression
MatchPattern ::= Literal
               | "is" Type

(* ── Littéraux ──────────────────────────────────────────────────── *)

Literal     ::= Integer | Float | String | TemplateString | Boolean | "null"
Integer     ::= Digit+
Float       ::= Digit+ "." Digit+
String      ::= '"' ( EscapeSeq | [^"\n] )* '"'
              | "'" ( EscapeSeq | [^'\n] )* "'"
Boolean     ::= "true" | "false"
EscapeSeq   ::= "\" ( "n" | "t" | "r" | '"' | "'" | "\" | "0" )

(* ── Identifiant ────────────────────────────────────────────────── *)

Identifier  ::= Letter ( Letter | Digit | "_" )*
Letter      ::= [a-zA-Z]
Digit       ::= [0-9]

(* ── Commentaires (ignorés par le parser) ───────────────────────── *)

LineComment ::= "//" [^\n]* "\n"
```