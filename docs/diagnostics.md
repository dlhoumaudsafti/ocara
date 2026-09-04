# Ocara — Diagnostics : erreurs et avertissements du compilateur

> Référence complète des messages produits par le compilateur `ocara` lors de la **compilation**.  
> Pour les exceptions runtime (IOException, MathException, etc.), voir la [section dédiée](#exceptions-runtime).

---

## Format des messages

Le compilateur suit la convention **GCC / clang**, reconnue par VS Code et la plupart des IDE.  
Chaque diagnostic est une ligne cliquable dans le terminal intégré :

```
fichier.oc:LIGNE:COL: error: message
fichier.oc:LIGNE:COL: warning: message
```

- **`error`** — bloque la compilation (exit code 1)
- **`warning`** — informatif, n'empêche pas la compilation

Les erreurs et avertissements sont triés par ligne/colonne et affichés ensemble.

**Langue des messages** : Tous les messages de diagnostic sont en **anglais**.

---

## Erreurs lexicales

Produites lors de la tokenisation du source.

| Message (anglais) | Cause | Exemple |
|---------|-------|---------|
| `Unexpected character 'X'` | Caractère non reconnu par la grammaire | `var x@ = 1` |
| `Unterminated string` | Guillemet ouvrant sans guillemet fermant | `var s = "bonjour` |
| `Invalid escape sequence '\X'` | `\X` inconnu dans une chaîne | `"\q"` |
| `Integer overflow: N` | Entier littéral dépassant `i64::MAX` | `var n = 99999999999999999999` |

---

## Erreurs syntaxiques (parse)

Produites lors de la construction de l'AST.

| Message | Cause |
|---------|-------|
| `expected ')'` | Parenthèse fermante manquante |
| `expected '}'` | Accolade fermante manquante |
| `expected ':'` | Déclaration de type manquante |
| `expected identifier` | Nom attendu mais token différent trouvé |
| `unexpected token 'X'` | Token inattendu à cette position |
| `operator 'X' has been removed — use 'Y' instead` | Ancien opérateur de comparaison symbolique (`==`, `!=`, `<`, `<=`, `>`, `>=`, `===`, `!==`, `<==`, `>==`) — voir [§11.1 de l'EBNF](EBNF.md#111-comparaisons) |

---

## Erreurs sémantiques

Produites lors de l'analyse de types et de symboles (`--check` ou compilation).

### E01 — Symbole indéfini

```
fichier.oc:5:10: error: undefined symbol 'foo'
```

Variable, fonction ou classe utilisée sans avoir été déclarée.

```ocara
var x:int = foo          // 'foo' n'existe pas
```

**Correction :** déclarer `foo` avant utilisation, ou corriger le nom.

---

### E02 — Incompatibilité de types

```
fichier.oc:7:5: error: expected type 'int', found 'string'
```

La valeur assignée ou retournée ne correspond pas au type déclaré.

```ocara
var n:int = "bonjour"    // string assigné à int
```

**Correction :** utiliser `Convert::strToInt()` ou corriger le type déclaré.

---

### E03 — Symbole en double

```
fichier.oc:12:5: error: duplicate symbol 'x'
```

Une variable ou fonction est déclarée deux fois dans le même scope.

```ocara
var x:int = 1
var x:int = 2            // doublon dans le même bloc
```

**Correction :** renommer l'une des deux déclarations.

---

### E04 — Symbole non appelable

```
fichier.oc:8:5: error: 'x' is not callable
```

Tentative d'appel d'une variable comme si c'était une fonction.

```ocara
var x:int = 42
x()                       // x n'est pas une fonction
```

---

### E05 — Mauvais nombre d'arguments

```
fichier.oc:9:5: error: 'IO::writeln' expects 1 argument(s), 3 provided
```

Appel d'une fonction avec un nombre d'arguments incorrect.

```ocara
IO::writeln("a", "b", "c")   // writeln n'attend qu'un seul argument
```

**Correction :** consulter la documentation de la fonction dans `docs/builtins/`.

---

### E06 — Type de retour incompatible

```
fichier.oc:15:5: error: expected return type 'int', found 'string'
```

La valeur retournée par une fonction ne correspond pas à son type de retour déclaré.

```ocara
function getId(): int {
    return "abc"          // doit retourner int
}
```

---

### E07 — Pas une classe

```
fichier.oc:20:15: error: 'MaVar' is not a class
```

Tentative d'instanciation (`use`) d'un symbole qui n'est pas une classe.

```ocara
var x:int = 42
var obj = use x()   // ❌ x n'est pas une classe
```

---

### E08 — Champ introuvable

```
fichier.oc:22:10: error: field 'nom' not found in class 'Point'
```

Accès à un champ ou méthode inexistant dans une classe.

```ocara
var p:Point = use Point(1, 2)
IO::writeln(p.nom)        // 'nom' n'est pas dans Point
```

---

### E09 — Interface non implémentée

```
fichier.oc:30:1: error: class 'Cercle' does not implement 'Forme::aire' from interface 'Forme'
```

Une classe déclare implémenter une interface mais n'en définit pas toutes les méthodes.

**Correction :** ajouter la méthode manquante dans la classe.

---

### E10 — Assignation invalide

```
fichier.oc:35:5: error: cannot assign to 'n' (immutable or undeclared)
```

Tentative de réaffectation d'un **paramètre de fonction/méthode** (toujours
immutable, quel que soit son type) ou d'une cible d'affectation invalide
(ex : une constante de classe accédée via `Classe::NOM`).

```ocara
function foo(n:int): int {
    n = 10           // ❌ un paramètre n'est jamais réaffectable
    return n
}

Math::PI = 3          // ❌ cible d'affectation invalide (constante statique)
```

> **`var`, `scoped` et `consumed` sont tous les trois mutables** —
> réaffectables librement après leur déclaration. Rien dans le langage
> aujourd'hui ne rend une variable locale immutable après coup ; seuls un
> paramètre ou une constante ne le sont jamais. Voir
> [§9 de l'EBNF](EBNF.md#9-variables-et-constantes) pour la portée et la
> politique de destruction de chacun — des sujets différents de la
> mutabilité.

---

### E11 — Méthode non statique appelée statiquement

```
fichier.oc:40:5: error: 'Compte::deposer' is not static — use an instance
```

Appel d'une méthode d'instance via `Classe::methode` au lieu d'une instance.

```ocara
Compte::deposer(500)      // deposer() n'est pas statique
```

**Correction :** créer une instance : `var c = use Compte(...); c.deposer(500)`.

---

### E12 — Méthode statique appelée sur une instance

```
fichier.oc:45:5: error: 'sqrt' is static — use self::sqrt() from within the class or Math::sqrt() from outside
```

Appel d'une méthode statique via une instance au lieu de la classe directement.

```ocara
var m = use Math()
m.sqrt(16.0)      // ❌ sqrt est statique
Math::sqrt(16.0)  // ✅ correct
```

---

### E13 — self hors contexte de classe

```
fichier.oc:50:5: error: internal error: self:: outside class context
```

Utilisation de `self` en dehors d'une méthode de classe.

```ocara
function libre(): void {
    self.x = 10   // ❌ self n'existe que dans les méthodes
}
```

**Correction :** `self` ne peut être utilisé que dans les méthodes d'instance.

---

### E14 — Type mixed interdit en property

```
fichier.oc:55:5: error: type 'mixed' is forbidden for class fields: 'User.data' must use a concrete type or 'map<string, mixed>'
```

Le type `mixed` ne peut pas être utilisé comme type de champ de classe (property).

```ocara
class User {
    public property data:mixed  // ❌ interdit
}
```

**Correction :** utiliser un type concret (`int`, `string`, `map<string, mixed>`, etc.) ou un type union (`int|string|null`).

---

### E15 — Type mixed interdit en retour de fonction

```
fichier.oc:60:1: error: type 'mixed' is forbidden as return type: 'getValue' must return a concrete type or use unions (e.g., int|string|null)
```

Le type `mixed` ne peut pas être utilisé comme type de retour de fonction ou méthode.

```ocara
function getValue(): mixed {  // ❌ interdit
    return 42
}
```

**Correction :** utiliser un type union explicite (`int|string|null`) ou un type concret.

---

### E16 — Comparaison entre types incompatibles

```
fichier.oc:5:16: error: cannot compare 'int' and 'string' with 'equal': comparisons are strictly typed (int and float are the only compatible pair) — convert one side explicitly
```

Une comparaison (`equal`, `not equal`, `smaller`, `greater`, `smaller or equal`,
`greater or equal`) porte sur deux types incompatibles. Depuis Ocara v0.2.0,
toute comparaison est vérifiée **à la compilation** dès que les deux types
sont statiquement connus — `int` et `float` sont l'unique paire compatible
malgré des types nominaux différents (widening numérique explicite, jamais un
bitcast) ; toute autre paire de types différents est rejetée. `smaller` /
`greater` / `smaller or equal` / `greater or equal` exigent en plus que les
deux types soient numériques (l'ordre n'a pas de sens pour un `bool` ou un
`string`).

```ocara
var n:int = 5
var s:string = "5"
if n equal s {           // ❌ int et string ne sont pas comparables
    ...
}

var b:bool = true
var i:int = 1
if b smaller i {         // ❌ smaller/greater n'accepte que des types numériques
    ...
}
```

**Correction :** convertir explicitement un des deux côtés (`Convert::intToStr`,
`Convert::strToInt`, ...) ou corriger le type de l'un des deux opérandes.

Une valeur `mixed` échappe à cette vérification statique (son type réel n'est
pas connu à la compilation) : la comparaison est alors déléguée à un contrôle
de type au runtime, plutôt qu'à ce diagnostic — voir [§11.1 de l'EBNF](EBNF.md#111-comparaisons).

---

### E17 — `consumed` utilisée deux fois

```
fichier.oc:6:17: error: 'x' is 'consumed' and was already used at 5:17 — it was destroyed right after that first use
```

Une variable `consumed` est détruite juste après sa toute première
utilisation — la réutiliser ensuite est une erreur de compilation, qui cite
la position de cette première utilisation. Voir [§9.3 de l'EBNF](EBNF.md#93-variable-à-usage-unique-consumed).

```ocara
consumed x:array<int> = [1, 2, 3]
IO::writeln(Array::len(x))   // 1ʳᵉ (et unique) utilisation — x détruit juste après
IO::writeln(Array::len(x))   // ❌ x n'existe déjà plus
```

**Correction :** n'utiliser `x` qu'une seule fois, ou passer à `scoped` si
plusieurs utilisations dans le même bloc sont nécessaires.

### E18 — Échappement d'une ressource `scoped`/`consumed`

```
fichier.oc:9:25: error: 'm' ('Mutex') cannot escape its 'scoped'/'consumed' block (assignment, return, or argument) — resource handles cannot be cloned or shared, use it locally via its own methods
```

Une `scoped`/`consumed` de type `Mutex`/`SQLite`/`MySQL`/`MariaDB`/`Thread`
est affectée à une variable, un champ, ou retournée — donc destinée à
survivre à son propre bloc. Contrairement à un `array`/`map` `scoped`/
`consumed` (silencieusement cloné dans ce cas), un handle de ressource ne
peut pas être dupliqué : deux « clones » d'un même `Mutex` ne protégeraient
plus la même section critique.

```ocara
scoped m:Mutex = use Mutex()
var leaked:Mutex = m   // ❌ m ne peut pas s'échapper de son bloc
```

**Correction :** garder l'usage de la ressource strictement local à son
bloc `scoped`/`consumed` (verrouiller/déverrouiller, requêter, etc. avant
la fin du bloc).

### E19 — `Thread` `scoped`/`consumed` non finalisée

```
fichier.oc:5:5: error: 't' is a 'scoped'/'consumed' Thread that reaches the end of its block without a call to '.join()' or '.detach()' — pick one explicitly
```

Une `scoped`/`consumed Thread` atteint la fin de son bloc sans avoir été
`.join()` (attendre sa fin) ni `.detach()` (la laisser tourner en tâche
de fond) — le compilateur ne peut pas choisir ce comportement à la place du
développeur, contrairement aux autres types ressource qui ont un
destructeur implicite unique.

**Correction :** appeler explicitement `.join()` ou `.detach()` sur la
`Thread` avant la fin de son bloc.

---

## Avertissements sémantiques

Les avertissements ne bloquent pas la compilation mais signalent du code suspect.

### W01 — Variable inutilisée

```
fichier.oc:14:5: warning: variable 'inutile' is never used
```

Une variable est déclarée mais sa valeur n'est jamais lue.

```ocara
var inutile:string = "jamais lue"   // déclarée mais pas utilisée
```

**Correction :** supprimer la variable ou l'utiliser. Les paramètres de fonctions sont exemptés de ce warning.

---

### W02 — Variable locale avec type mixed

```
fichier.oc:18:5: warning: local variable 'temp': type 'mixed' disables type checking — prefer a concrete type or union (e.g., int|string|null)
```

Une variable locale utilise le type `mixed`, ce qui désactive la vérification de types.

```ocara
var temp:mixed = getValue()  // ⚠️ warning
```

**Correction :** utiliser un type concret ou un type union (`int|string|null`).

---

### W03 — Paramètre variadique avec type mixed

```
fichier.oc:22:5: warning: variadic parameter 'args': variadic<mixed> disables type checking — consider variadic<T|U> with explicit union
```

Un paramètre variadique utilise `mixed`, ce qui désactive la vérification de types.

```ocara
function log(args:variadic<mixed>): void {  // ⚠️ warning
    // ...
}
```

**Correction :** utiliser un type union explicite pour le variadique.

---

## Utilisation

### Vérification sans compilation

```bash
./target/release/ocara mon_fichier.oc --check
```

Affiche toutes les erreurs et warnings sans produire de binaire.

### Voir l'exemple de référence

Le fichier `examples/21_errors.oc` déclenche volontairement plusieurs erreurs et warnings :

```bash
./target/release/ocara examples/21_errors.oc --check
```

---

## Exceptions runtime

Les erreurs ci-dessus sont des **erreurs de compilation** détectées avant l'exécution.

Les **exceptions runtime** (levées pendant l'exécution du programme) sont documentées dans les pages des builtins correspondants :

| Exception | Builtins concernés | Documentation |
|-----------|-------------------|---------------|
| `IOException` | IO, File, Directory | [IO.md](builtins/IO.md) |
| `MathException` | Math | [Math.md](builtins/Math.md) |
| `SystemException` | System | [System.md](builtins/System.md) |
| `RegexException` | Regex | [Regex.md](builtins/Regex.md) |
| `ArrayException` | Array | [Array.md](builtins/Array.md) |
| `MapException` | Map | [Map.md](builtins/Map.md) |
| `ThreadException` | Thread | [Thread.md](builtins/Thread.md) |
| `MutexException` | Mutex | [Mutex.md](builtins/Mutex.md) |
| `HTTPException` | HTTPRequest, HTTPServer | [HTTPRequest.md](builtins/HTTPRequest.md) |
| `YAMLException` | YAML | [YAML.md](builtins/YAML.md) |
| `SQLiteException` | SQLite | [SQLite.md](builtins/SQLite.md) |
| `MySQLException` / `MariaDBException` | MySQL / MariaDB | [MySQL.md](builtins/MySQL.md) |
| `DotEnvException` | DotEnv | [DotEnv.md](builtins/DotEnv.md) |
| `TauriException` | Tauri | [Tauri.md](builtins/Tauri.md) |
| `SDLException` | SDL | [SDL.md](builtins/SDL.md) |

Chaque exception a des **codes d'erreur spécifiques** (101, 102, etc.) documentés dans les pages correspondantes.

**Gestion des exceptions** :

```ocara
try {
    var result:int = Math::sqrt(-4.0)
} on e is MathException {
    IO::writeln(`Math error: ${e.message}`)
    IO::writeln(`Code: ${e.code}`)
} on e {
    // catch-all pour toute exception
    IO::writeln(`Unexpected error: ${e.message}`)
}
```

---

## Codes de sortie du compilateur

| Code | Signification |
|------|--------------|
| `0` | Succès — aucune erreur de compilation |
| `1` | Erreur(s) de compilation — analyse ou codegen échouée |
