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

### E20 — Concaténation `string` + type différent

```
fichier.oc:5:27: error: cannot concatenate 'string' and 'int' with '+': string concatenation is strictly typed (only string + string is allowed) — use a template string (`${...}`) or convert explicitly (Convert::*ToStr)
```

`+` sur `string` mélange un `string` avec un type différent (`int`, `float`,
`bool`, `array<T>`, `map<K,V>`, une classe...). La concaténation `+` est
strictement typée : **seule `string + string` produit un `string`** — il n'y a
pas de conversion implicite d'un autre type vers `string` via `+`, ni dans un
sens ni dans l'autre. Voir [§11.2 de l'EBNF](EBNF.md#112-concaténation--sur-string).

```ocara
var n:int = 42
IO::writeln("Total : " + n)   // ❌ string et int ne se concatènent pas directement
```

**Correction :** utiliser un template string (conversion automatique de
n'importe quel type interpolé), ou convertir explicitement un des deux côtés
(`Convert::intToStr`, `Convert::floatToStr`, `Convert::boolToStr`, ...) :

```ocara
IO::writeln(`Total : ${n}`)                       // ✅ template string
IO::writeln("Total : " + Convert::intToStr(n))    // ✅ conversion explicite
```

Une valeur `mixed` échappe à cette vérification statique (son type réel n'est
pas connu à la compilation), comme pour E16 : `string + mixed` est délégué à
un rendu au runtime plutôt qu'à ce diagnostic.

### E21 — Arité incorrecte des arguments de type d'un générique

```
fichier.oc:5:14: error: generic 'List' expects 1 type argument(s), 3 provided
```

Le nombre d'arguments de type passés à `use Foo<...>()` ne correspond pas au
nombre de paramètres de type déclarés par `generic Foo<T, U=default>` — entre
le nombre de paramètres **sans** valeur par défaut (minimum) et le nombre
total de paramètres déclarés (maximum, défauts inclus).

```ocara
generic List<T> { ... }

var l:List<int> = use List<int>()          // ✅ 1 attendu, 1 fourni
var m:List<int,string,Foo> = use List<int,string,Foo>()   // ❌ 1 attendu, 3 fournis
var n:List = use List()                     // ❌ 1 attendu, 0 fourni
```

**Correction :** fournir exactement le nombre d'arguments de type attendu par
la déclaration `generic`.

### E22 — `Thread` déjà finalisée

```
fichier.oc:8:6: error: 't' was already '.join()'ed or '.detach()'ed — calling either a second time would use a native handle already reclaimed
```

`.join()` ou `.detach()` est appelé une seconde fois sur la même `Thread` —
dans n'importe quelle combinaison (`join` puis `join`, `join` puis `detach`,
...). Le premier appel a déjà repris et libéré le handle natif côté runtime
(`runtime/src/thread.rs`) ; un second appel utiliserait ce même pointeur déjà
invalidé — use-after-free confirmé (abort immédiat à l'exécution avant ce
diagnostic).

```ocara
scoped t:Thread = use Thread()
t.run(nameless(): void { ... })
t.join()
t.join()   // ❌ 't' déjà finalisée
```

**Correction :** appeler `.join()` ou `.detach()` une seule fois par `Thread`.

---

### E23 — Classe de filtre `on ... is` introuvable

```
fichier.oc:8:7: error: 'TypoException' is not a known class — this 'on e is TypoException' handler would never match anything
```

`on e is X` où `X` ne correspond à aucune classe connue (ni classe utilisateur du programme, ni classe d'exception builtin — celles-ci sont toutes reconnues sans import explicite). Un typo rendait jusqu'ici ce handler silencieusement mort : aucun `raise` ne peut jamais lui correspondre, sans le moindre avertissement.

```ocara
try {
    raise "boom"
} on e is TypoException {   // ❌ 'TypoException' n'existe pas
    IO::writeln("jamais atteint")
}
```

**Correction :** corriger le nom de la classe, ou déclarer la classe si elle doit être définie par le programme.

---

### E24 — Handler catch-all mal positionné

```
fichier.oc:9:7: error: a catch-all 'on' handler (without 'is') must be the last one in this try/on chain — handlers after it would never be reached
```

Un handler `on e { }` (sans `is`, donc catch-all) apparaît avant un autre handler dans la même chaîne `try`/`on` — il filtre déjà tout, rendant les handlers suivants inatteignables (voir docs/EBNF.md §28.2, qui impose cet ordre).

```ocara
try {
    File::read("/nope")
} on e {
    IO::writeln("générique")
} on e is FileException {   // ❌ jamais atteint : le catch-all précédent absorbe déjà tout
    IO::writeln("spécifique")
}
```

**Correction :** placer le handler catch-all en dernier dans la chaîne.

---

### E25 — Ressource déjà finalisée manuellement

```
fichier.oc:9:6: error: 'm' ('Mutex') was already '.destroy()'ed — calling it a second time would use a native handle already reclaimed
```

`.destroy()` (`Mutex`) ou `.close()` (`SQLite`/`MySQL`/`MariaDB`) est appelé une seconde fois sur la même ressource — généralisation de E22 (`Thread`). Le premier appel a déjà libéré le handle natif côté runtime ; un second appel produit un SEGFAULT confirmé avant ce diagnostic.

```ocara
scoped m:Mutex = use Mutex()
m.destroy()
m.destroy()   // ❌ 'm' déjà finalisée
```

**Correction :** appeler `.destroy()`/`.close()` une seule fois par ressource.

---

### E26 — Argument `scoped`/`consumed` qui s'échappe

```
fichier.oc:9:19: error: 'arr' ('array<int>') is passed as an argument to 'Box::init', which stores it beyond this call — a 'scoped'/'consumed' value cannot be passed where the callee retains it; clone it explicitly first, or pass a fresh value
```

Une `scoped`/`consumed` passée en argument d'un appel (constructeur, méthode) qui la stocke au-delà de l'appel (ex. un constructeur qui affecte le paramètre à un champ) — la source est libérée en fin de bloc alors que l'appelé en garde encore un alias : pointeur pendouillant, corruption mémoire silencieuse avant ce diagnostic (voir docs/roadmap.d/memoire-echappement-argument.md).

```ocara
class Box {
    public property data:array<int>
    init(a:array<int>) { self.data = a }
}
function makeBox(): Box {
    scoped arr:array<int> = [111, 222, 333]
    var b:Box = use Box(arr)   // ❌ 'arr' sera libérée en fin de bloc
    return b
}
```

Pour une `scoped`/`consumed` de type ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`/`Thread`), tout passage en argument est rejeté (`ResourceEscape`, pas de distinction retenu/prêté possible pour une ressource). Pour `string`/`array`/`map`/instance de classe utilisateur, seul un appel vers une fonction/méthode/constructeur **utilisateur** connue dont ce paramètre est prouvé retenu est rejeté — `Array::push(arr, x)`/`Map::set(m, k, v)` (mutation en place) restent autorisés.

**Correction :** cloner explicitement avant l'appel (ex. `arr.slice(0, arr.len())`), ou déclarer la variable en `var` si le partage est voulu.

---

### E27 — `extends` vers une classe/generic inconnu

```
fichier.oc:9:1: error: class 'Foo' extends unknown class 'DoesNotExist'
fichier.oc:5:1: error: generic 'Bag' extends unknown class/generic 'NoSuchThing'
```

Une classe ou un `generic` déclare `extends X` où `X` ne correspond à aucune classe (ni, pour un `generic`, à aucun autre `generic`) connue — auparavant accepté silencieusement, sans que l'héritage ne fasse quoi que ce soit d'utile.

**Correction :** corriger le nom du parent, ou retirer la clause `extends` si elle n'était pas voulue.

---

### E28 — Fuite d'un handle natif déclaré en `var`/`const`

```
fichier.oc:4:5: error: 'm' ('Mutex') is declared with 'var'/'const', never escapes its block, and is never '.destroy()'ed/'.close()'d — this native handle leaks permanently, since 'var'/'const' never close a resource automatically (unlike 'scoped'/'consumed'); call '.destroy()'/'.close()' explicitly, or declare it 'scoped'/'consumed' if you want the compiler to finalize it for you
```

Un `var`/`const` d'un type ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`) dont l'analyse d'échappement statique (la même que pour la libération automatique d'un `var`, voir `crate::sema::escape::var_never_escapes`) prouve qu'il ne s'échappe jamais (jamais retourné, réaffecté, ni passé en argument), et qui atteint la fin de son bloc sans avoir été manuellement `.destroy()`/`.close()`. Contrairement à `scoped`/`consumed`, qui finalisent automatiquement une ressource en fin de bloc, `var`/`const` ne le font jamais — ce handle natif (mutex, connexion) fuit alors pour toujours.

Volontairement conservateur : dès que la variable pourrait s'échapper d'une façon quelconque (retour, réaffectation, argument d'un appel), aucune erreur n'est levée — mieux vaut manquer une fuite réelle que rejeter du code légitime.

```ocara
function main(): int {
    var m:Mutex = use Mutex()   // ❌ jamais fermé, jamais échappé
    m.lock()
    m.unlock()
    return 0
}
```

**Correction :** appeler `.destroy()`/`.close()` explicitement avant la fin du bloc, ou déclarer la variable `scoped`/`consumed` si la fermeture automatique de fin de bloc est voulue.

---

### E29 — Champ de classe d'un type ressource

```
fichier.oc:5:5: error: 'Cache.lock' ('Mutex') is a native resource field — it is never closed when a 'Cache' instance is destroyed (no mechanism exists for this today), so this handle always leaks; manage it outside the class instead, or expose an explicit method the caller must invoke before discarding the instance
```

Une `property` d'un type ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`) sur une classe utilisateur — `__free_<Classe>` (généré pour `scoped`/`consumed`, et pour un `var` auto-libéré) ne sait libérer/fermer qu'un champ `string`/`array`/`map`/instance de classe utilisateur, jamais une ressource : ce champ fuirait systématiquement son handle natif à chaque libération de l'instance porteuse, quelle que soit la façon dont cette instance est elle-même gérée.

**Correction :** ne pas stocker la ressource directement dans un champ de la classe — la gérer en dehors (ex. l'injecter à chaque appel de méthode plutôt que de la conserver), ou exposer une méthode explicite (`close()`) que l'appelant doit invoquer lui-même avant d'abandonner l'instance.

Lever cette restriction est une réflexion ouverte, non tranchée — voir [langage-destructeur-champ-ressource](roadmap.d/langage-destructeur-champ-ressource.md).

---

### E30 — `message<T>` nommé (`var`/`scoped`/`consumed`)

```
fichier.oc:5:5: error: 'm': type 'message<T>' cannot be named — it is valid only as the declared return type of a function/method containing 'emit', never in a 'var'/'scoped'/'consumed' declaration
```

`message<T>` (générateurs, voir `docs/roadmap.d/langage-emit-iterable.md`) n'est jamais nommable : il n'existe que comme résultat anonyme et immédiat d'un appel à une fonction/méthode contenant `emit`.

```ocara
function truc(): message<int> { emit 1 }
consumed m:message<int> = truc()   // ❌ E30
```

**Correction :** consommer le `message<T>` directement (`for x in truc()`, `var x:int = truc()`, `Array::fromMessage(truc())`) sans jamais le nommer lui-même.

---

### E31 — `message<T>` comme type de paramètre

```
fichier.oc:3:19: error: parameter 'm': type 'message<T>' cannot be used as a parameter type — it is return-type-only
```

`message<T>` est return-type-only : il ne peut jamais être le type déclaré d'un paramètre de fonction, méthode ou constructeur.

**Correction :** ne pas passer de `message<T>` en paramètre — la fonction qui produit les valeurs doit elle-même contenir les `emit`.

---

### E32 — `message<T>` en retour sans `emit`

```
fichier.oc:3:1: error: 'noEmit' declares return type 'message<T>' but its body contains no reachable 'emit' — 'message<T>' is only valid as the return type of a function/method that actually emits
```

Une fonction/méthode déclare `message<T>` en retour mais son corps ne contient aucun `emit` atteignable — `message<T>` n'a de sens que pour une fonction qui émet réellement (pas de transfert/forwarding pris en charge).

**Correction :** ajouter au moins un `emit` au corps, ou changer le type de retour si la fonction n'est pas un générateur.

---

### E33 — `emit` hors d'une fonction `message<T>`

```
fichier.oc:4:5: error: 'emit' is only valid inside a function/method whose declared return type is 'message<T>'
```

`emit` est utilisé dans une fonction/méthode dont le type de retour déclaré n'est pas `message<T>`.

**Correction :** déclarer le type de retour de la fonction en `message<T>`, ou retirer le `emit`.

---

### E34 — Consommation scalaire directe d'un `message<T>` multi-émission

```
fichier.oc:8:5: error: 'trucLoop()' returns 'message<T>' with an 'emit' reachable inside a loop — the compiler cannot prove that at most one value is ever produced, so it cannot be consumed directly as a scalar here; use 'for x in trucLoop()' or 'Array::fromMessage(trucLoop())' instead
```

`emit` dans une boucle (`while`/`for`) reste du Ocara parfaitement valide, mais désactive la consommation scalaire directe (`var x:T = f()`, argument de fonction) : le compilateur ne peut plus prouver statiquement qu'au plus une valeur est jamais produite. Le branchement simple (`if`/`elseif`/`else`, `switch`) n'est PAS concerné — `if cond { emit 1 } else { emit 2 }` reste consommable directement.

```ocara
function trucLoop(): message<int> {
    var i:int = 0
    while i smaller 3 { emit i; i = i + 1 }
}
var v:int = trucLoop()   // ❌ E34
for x in trucLoop() { }  // ✅ toujours valable
```

**Correction :** consommer via `for x in ...` ou `Array::fromMessage(...)` plutôt qu'en scalaire direct.

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

### W04 — Ressource `scoped`/`consumed` ouverte au moment d'un `raise` non rattrapé localement

```
fichier.oc:5:5: warning: 'm' ('Mutex') is a 'scoped'/'consumed' resource still open when a 'raise' later in this block is not locally caught — its 'longjmp' skips this block's normal cleanup, leaking 'm' (or leaving a Mutex locked forever) — finalize it ('.destroy()'/'.close()'/'.join()'/'.detach()') before that 'raise', or wrap the risky code in a local 'try'/'on'
```

Une `scoped`/`consumed` ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`/`HTTPRequest`/`HTTPResponse`/`Thread`) est encore ouverte (pas finalisée manuellement) quand un `raise` plus loin dans le même bloc n'est protégé par aucun `try` local. Le mécanisme d'exceptions d'Ocara (`setjmp`/`longjmp`) saute par-dessus la finalisation automatique de fin de bloc — la ressource fuit (ou, pour un `Mutex`, reste verrouillée pour toujours) si ce `raise` se déclenche réellement à l'exécution. Voir [§9.2 de l'EBNF](EBNF.md#92-variable-de-bloc-scoped) et `docs/roadmap.d/exceptions-setjmp-longjmp-dette.md`.

```ocara
scoped m:Mutex = use Mutex()
m.lock()
raise use MonException("erreur", 1)   // ⚠️ 'm' ne sera jamais déverrouillé/détruit
m.unlock()                             // jamais atteint
```

Volontairement **conservateur** (mêmes principes que E26/E28) : un `raise` à l'intérieur d'un `try` local (même sans vérifier que ses `on` couvrent la classe réellement levée) est considéré rattrapé, jamais signalé ; une finalisation (`.destroy()`/`.close()`/`.join()`/`.detach()`) appelée en ligne droite avant le `raise` supprime l'avertissement. Aucune analyse interprocédurale : seul un `raise` textuel compte, pas un appel vers une fonction qui pourrait elle-même en lever un.

**Correction :** finaliser la ressource avant le code risqué, ou utiliser une variante `withX` qui garantit la finalisation même en cas d'exception — `m.withLock(...)` (voir [Mutex](builtins/Mutex.md)), `SQLite::withOpen(...)` (voir [SQLite](builtins/SQLite.md)), `MySQL::withConnect(...)`/`MariaDB::withConnect(...)` (voir [MySQL](builtins/MySQL.md)) — ou entourer le code à risque d'un `try`/`on` local.

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
