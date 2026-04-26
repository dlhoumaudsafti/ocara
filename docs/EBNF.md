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
13. [Fonctions builtins](#13-fonctions-builtins)
14. [Classes](#14-classes)
15. [Interfaces](#15-interfaces)
16. [Héritage et implémentation](#16-héritage-et-implémentation)
17. [Instanciation](#17-instanciation)
18. [Accès statique](#18-accès-statique)
19. [Conditions](#19-conditions)
20. [Switch](#20-switch)
21. [Match (expression)](#21-match-expression)
22. [Boucles](#22-boucles)
23. [Gestion des erreurs](#23-gestion-des-erreurs)
24. [Résolution des noms](#24-résolution-des-noms)
25. [Grammaire EBNF complète](#25-grammaire-ebnf-complète)
26. [Exemple complet](#26-exemple-complet)

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
            ( ConstDecl | ClassDecl | InterfaceDecl | FuncDecl )*
```

**Contraintes d'ordre :**
- Les imports sont toujours en tête de fichier.
- Les déclarations de constantes, classes, interfaces et fonctions peuvent être dans n'importe quel ordre entre elles.
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
| `mixed`  | Type dynamique, accepte toute valeur — **à utiliser avec parcimonie** |
| `void`   | Absence de valeur (retour seulement) |

> **Avertissement `mixed`** : le type `mixed` désactive la vérification de type statique. Préférer les types concrets chaque fois que possible.

### 4.2 Types composites

```ebnf
Type ::= "int"
       | "float"
       | "string"
       | "bool"
       | "mixed"
       | "void"
       | "Function"
       | ArrayType
       | MapType
       | QualifiedType
       | UnionType
       | Identifier

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

### 4.4 Annotation de type

Les variables et paramètres sont obligatoirement annotés :

```ocara
var x:int = 5
scoped name:string = "Alice"
function greet(name:string): void { }
```

### 4.5 Type `Function`

Le type `Function` représente toute valeur appelable : **fonction libre**, **méthode statique** ou **fonction anonyme** (`nameless`). Les valeurs `Function` sont des *fat pointers* (pointeur de fonction + contexte de capture).

```ebnf
FunctionType ::= "Function"
```

```ocara
var f:Function = double                 // référence à une fonction libre
var g:Function = MathOp::square        // référence à une méthode statique
var h:Function = nameless(x:int): int { return x * 2 }  // fonction anonyme
```

**Règles :**

- `Function` peut référencer des **fonctions libres**, des **méthodes statiques** et des **fonctions anonymes** (`nameless`).
- L'appel d'une valeur `Function` utilise la syntaxe d'appel normale : `f(args...)`.
- Le type de retour et les types de paramètres ne sont **pas** encodés dans `Function` — la compatibilité est vérifiée à l'exécution.
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
nameless
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
               | "use" Identifier "(" ArgList? ")"
               | Identifier "::" Identifier "(" ArgList? ")"
               | Identifier "::" Identifier
               | Identifier
               | MatchExpr
               | ArrayLiteral
               | MapLiteral
               | "(" Expression ")"
               | NamelessExpr

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
FuncDecl ::= "function" Identifier "(" ParamList? ")" ":" Type Block

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

## 13. Fonctions builtins (dépréciées)

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

---

## 14. Classes

```ebnf
ClassDecl  ::= "class" Identifier
               ( "extends" Identifier )?
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

### 14.1 Constructeur (`init`)

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

## 17. Instanciation

```ebnf
NewExpr ::= "use" Identifier "(" ArgList? ")"
```

Le mot-clé `use` appelle le constructeur `init` de la classe.

```ocara
var user:User = use User("David", 43)
var logger:Logger = use ConsoleLogger()
```

---

## 18. Accès statique

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

## 19. Conditions

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

## 20. Switch

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

## 21. Match (expression)

```ebnf
MatchExpr ::= "match" PostfixExpr "{" MatchArm+ "}"

MatchArm ::= Literal "=>" Expression
           | "default" "=>" Expression
```

`match` est une **expression** (retourne une valeur). Chaque bras produit une valeur.

```ocara
scoped label:string = match score {
    100 => "parfait"
    90  => "excellent"
    default => "insuffisant"
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

## 22. Boucles

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

## 23. Gestion des erreurs

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

## 24. Résolution des noms

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

## 25. Grammaire EBNF complète

> Notation : `*` = zéro ou plus, `+` = un ou plus, `?` = optionnel, `|` = alternative, `( )` = groupement.

```ebnf
(* ── Programme ─────────────────────────────────────────────────── *)

Program     ::= ImportDecl*
                ( ConstDecl | ClassDecl | InterfaceDecl | FuncDecl )*

(* ── Imports ────────────────────────────────────────────────────── *)

ImportDecl  ::= "import" ModulePath ( "as" Identifier )?
ModulePath  ::= Identifier ( "." Identifier )*

(* ── Déclarations globales ──────────────────────────────────────── *)

ConstDecl   ::= "const" Identifier ":" Type "=" Expression
ClassDecl   ::= "class" Identifier
                ( "extends" Identifier )?
                ( "implements" Identifier ( "," Identifier )* )?
                ClassBody
InterfaceDecl ::= "interface" Identifier "{" InterfaceMethod* "}"
FuncDecl    ::= "function" Identifier "(" ParamList? ")" ":" Type Block

(* ── Classe ─────────────────────────────────────────────────────── *)

ClassBody   ::= "{" ClassMember* "}"
ClassMember ::= Constructor
              | Visibility "static"? "method" Identifier "(" ParamList? ")" ":" Type Block
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

Type        ::= "int" | "float" | "string" | "bool" | "mixed" | "void" | "Function"
              | ArrayType
              | MapType
              | QualifiedType
              | UnionType
              | Identifier
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
UnaryExpr   ::= ( "not" | "-" ) UnaryExpr | PostfixExpr
PostfixExpr ::= PrimaryExpr PostfixTail*
PostfixTail ::= "." Identifier ( "(" ArgList? ")" )?
              | "(" ArgList? ")"
              | "[" Expression "]"

PrimaryExpr ::= Literal
              | "self"
              | NewExpr
              | StaticCall
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

MatchExpr   ::= "match" PostfixExpr "{" MatchArm+ "}"
MatchArm    ::= Literal "=>" Expression
              | "default" "=>" Expression

(* ── Littéraux ──────────────────────────────────────────────────── *)

Literal     ::= Integer | Float | String | Boolean
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

---

## 26. Exemple complet

```ocara
import datas.User as UserData
import roles.User as UserAcl
import repository.User
import services.Logger


const TAX:float = 0.2


class User {

    private property name:string
    protected property age:int

    init(name:string, age:int) {
        self.name = name
        self.age = age
    }

    public method greet(): void {
        IO::writeln("Hello " + self.name)
    }
}


interface Logger {
    method log(msg:string): void
}


class ConsoleLogger implements Logger {

    public method log(msg:string): void {
        IO::writeln(msg)
    }
}


function main(): int {

    var user:UserData = use UserData("David", 43)

    user.greet()


    scoped status:string = match user.age:int {
        43 => "old"
        20 => "young"
        default => "unknown"
    }

    IO::writeln(status)


    switch user.age:int {
        43 {
            IO::writeln("exact match")
        }
        default {
            IO::writeln("no match")
        }
    }


    var logger:Logger = use ConsoleLogger()
    logger.log("system started")


    for i in 0..5 {
        IO::writeln(i)
    }


    // Tableaux
    var numbers:int[] = [1, 2, 3]
    IO::writeln(numbers[0])

    var matrix:int[][] = [
        [1, 2],
        [3, 4]
    ]

    var vals:mixed[] = [1, "hello", true]
    IO::writeln(vals[1])


    // Tableau associatif
    var profile:map<string, mixed> = {
        "name": "David",
        "age":  43
    }
    IO::writeln(profile["name"])

    for key => val in profile {
        IO::writeln(key)
        IO::writeln(val)
    }


    // Lecture clavier
    IO::writeln("Quel est ton nom ?")
    scoped input:string = IO::read()
    IO::writeln("Bonjour " + input)


    return 0
}
```
