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
| `unexpected top-level declaration: Wiring` | `wiring` utilisé en dehors d'un corps `interface` (déclaration de premier niveau, ou à l'intérieur d'une classe/fonction) — `wiring` n'est reconnu que par `parse_interface`, voir [§17.1 de l'EBNF](EBNF.md#171-wiring--liaison-interface--implémentation-à-la-compilation) |

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
fichier.oc:8:6: error: 't' was already '.join()' or '.detach()' — calling either a second time would use a native handle already reclaimed
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
fichier.oc:9:6: error: 'm' ('Mutex') was already '.destroy()' — calling it a second time would use a native handle already reclaimed
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
fichier.oc:4:5: error: 'm' ('Mutex') is declared with 'var'/'const', never escapes its block, and is never '.destroy()'/'.close()' — this native handle leaks permanently, since 'var'/'const' never close a resource automatically (unlike 'scoped'/'consumed'); call '.destroy()'/'.close()' explicitly, or declare it 'scoped'/'consumed' if you want the compiler to finalize it for you
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

### E29 — (retiré) Champ de classe d'un type ressource

Historiquement, une `property` d'un type ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`/`HTTPRequest`/`HTTPResponse`) sur une classe utilisateur était rejetée d'office (`__free_<Classe>` ne savait libérer qu'un champ `string`/`array`/`map`/instance de classe, jamais une ressource).

**Ce n'est plus le cas.** `__free_<Classe>` ferme désormais un tel champ automatiquement, via son symbole runtime dédié (`SQLite_close`, `Mutex_destroy`, ...), quand l'instance porteuse est détruite (`scoped`/`consumed`, ou un `var`/`const` prouvé non-échappant). Pour que ça reste sûr, la classe porteuse est alors traitée comme une ressource NATIVE partout où l'échappement est vérifié :

- Une `scoped`/`consumed` instance de cette classe ne peut pas s'échapper de son bloc (assignation, `return`, argument) — voir E18 : deux instances vivantes ne peuvent jamais se partager le même handle (`__clone_<Classe>` ne clone jamais un champ ressource).
- Un `var`/`const` de cette classe doit être fermé manuellement ou prouvé non-échappant, sinon E28 (« fuite permanente ») s'applique — comme pour une ressource nue, sauf qu'une classe composite n'a en général pas de méthode `close()` à elle : `scoped`/`consumed` reste le seul choix pratique.
- Voir E35 ci-dessous pour l'interdiction de fermer manuellement un tel champ depuis une méthode de la classe.

Cas d'usage typique : `examples/advanced/tauri_httpserver/configs/Database.oc`, une classe `Database` qui garde sa connexion `SQLite` ouverte comme champ d'instance, ouverte dans `init()`. Voir [langage-destructeur-champ-ressource](roadmap.d/langage-destructeur-champ-ressource.md) pour l'historique de cette décision.

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

### E35 — Fermeture manuelle d'un champ ressource possédé par la classe

```
fichier.oc:12:16: error: 'self.db' ('SQLite') is closed automatically when the 'Database' instance is destroyed — calling '.close()' manually here would close it a second time (undefined behavior); remove this call
```

Depuis qu'une `property` de type ressource est autorisée sur une classe utilisateur (voir E29 ci-dessus), `__free_<Classe>` ferme ce champ automatiquement à la destruction de l'instance porteuse. Appeler `.close()`/`.destroy()`/`.closeResponse()` manuellement sur `self.<champ>` depuis une méthode de la classe referait donc TOUJOURS cette fermeture une seconde fois — contrairement à E25 (`ResourceAlreadyFinalized`), qui ne détecte qu'un second appel explicite sur une variable locale, ici le PREMIER appel explicite est déjà en trop : aucun suivi n'est possible à travers des appels de méthode arbitraires pour distinguer un usage sûr.

```ocara
class Database {
    private property db:SQLite

    init() {
        self.db = SQLite::open("./app.db")
    }

    public method migrate(): void {
        self.db.execute("CREATE TABLE IF NOT EXISTS visits (id INTEGER)")
        self.db.close()   // ❌ E35 — déjà fermé automatiquement à la destruction de l'instance
    }
}
```

**Correction :** retirer l'appel manuel — la fermeture est déjà prise en charge par le destructeur généré de la classe.

---

### E36 — Appel de méthode chaîné sur un récepteur `void`

```
fichier.oc:8:13: error: cannot call '.workers(...)' — the receiver's type is 'void' (likely the return value of a preceding chained call); a method that returns 'void' cannot be chained, since there is nothing to call '.workers(...)' on
```

`expr.méthode(...)` où `expr` est elle-même le résultat d'un appel dont le type de retour déclaré est `void` — typiquement `self.port(8080).workers(4)`, `port()` ne retournant rien à chaîner. `void` signifiant ICI « aucune valeur produite », enchaîner un appel dessus n'a jamais de sens, quel que soit le nom de méthode appelé ensuite.

```ocara
class Server {
    public method port(p:int): void {
        // ...
    }
    public method workers(n:int): void {
        // ...
    }
}

function main(): int {
    var s:Server = use Server()
    s.port(8080).workers(4)   // ❌ E36 — port() retourne void, rien à chaîner
    return 0
}
```

Avant ce diagnostic, un récepteur de type `void` était traité comme n'importe quel autre type sans classe associée (silencieusement permissif, retournait `Type::Mixed` sans vérifier le reste de la chaîne) : `.workers(4)` manglait alors vers un symbole inexistant, ignoré silencieusement par le codegen — `workers()` n'était en réalité JAMAIS appelée, sans la moindre erreur de compilation.

**Correction :** séparer les appels, chacun sur sa propre ligne (`s.port(8080)` puis `s.workers(4)`) — ce n'est PAS une limitation à contourner en faisant retourner `self` depuis `port()`/`workers()` : voir docs/roadmap.d/langage-appel-methode-sur-void-accepte.md pour pourquoi ce style « fluide » n'a pas été retenu pour ce langage.

---

### E37 — Appel de méthode sur un récepteur sans classe associée (`int`/`float`/`bool`/`null`/`message<T>`/`Function<...>`)

```
fichier.oc:11:32: error: cannot call '.upper(...)' — the receiver's type is 'int', which has no methods
```

`expr.méthode(...)` où `expr` est de type `int`, `float`, `bool`, `null`, `message<T>` ou `Function<...>` — aucun de ces types n'a de classe associée, donc aucune méthode ne peut exister dessus. Même mécanisme que E36 (void), généralisé aux types que ce correctif-là avait délibérément laissés de côté.

```ocara
class Foo {
    public method getCount(): int {
        return 42
    }
}

function main(): int {
    var f:Foo = use Foo()
    var r:string = f.getCount().upper()   // ❌ E37 — getCount() retourne int, .upper() n'existe sur aucun int
    return 0
}
```

Avant ce diagnostic, un récepteur de l'un de ces types était traité comme n'importe quel autre type sans classe associée (silencieusement permissif, retournait `Type::Mixed` sans vérifier `field`/`args`) : `.upper()` manglait vers un symbole inexistant, ignoré silencieusement par le codegen — le programme ci-dessus affichait `null` à l'exécution au lieu d'être rejeté à la compilation.

**Important :** `mixed` n'est **pas** concerné par ce diagnostic — son imprécision (aucune vérification de type) est un choix de langage assumé, documenté par l'avertissement W02 (voir plus bas), pas un oubli comme les types ci-dessus.

**Correction :** ne pas appeler de méthode sur un récepteur de l'un de ces types — s'assurer que la méthode précédente de la chaîne retourne bien une instance de classe (ou `string`/`array`/`map`, qui ont leurs propres méthodes d'instance sucrées) avant de chaîner un appel dessus.

---

### E38 — Construction/appel statique sur une interface sans `wiring`

```
fichier.oc:5:18: error: interface 'Repo' cannot be constructed or have a static method called on it directly: it has no 'wiring' declaration — add at least one 'wiring <Class>' inside the interface, or use a concrete implementing class directly
```

`use Interface(...)` ou `Interface::méthode(...)` sur le nom **nu** (sans alias) d'une interface qui ne déclare **aucun** `wiring` — voir [§17.1 de l'EBNF](EBNF.md#171-wiring--liaison-interface--implémentation-à-la-compilation) et `docs/roadmap.d/langage-interface-wiring.md`. Une interface reste un contrat abstrait : sans `wiring`, il n'existe aucune classe concrète vers laquelle résoudre la construction/l'appel.

```ocara
interface Repo {
    method save(): void
}

function main(): int {
    var r:Repo = use Repo()   // ❌ E38 — Repo n'a aucun `wiring`
    return 0
}
```

Avant ce diagnostic, `Interface::méthode()` (appel statique) était silencieusement permissif : `lookup_method_in_chain` ne cherche que dans les classes, jamais dans les interfaces — l'appel retombait sur `Type::Mixed` sans la moindre erreur, un résultat **faux silencieux** plutôt qu'un rejet clair. `use Interface(...)` (construction), lui, était déjà rejeté, mais avec le message générique E07 (« not a class »), moins parlant que ce diagnostic dédié.

**Correction :** ajouter au moins un `wiring <Classe>` dans le corps de l'interface, ou construire/appeler directement une classe concrète qui l'implémente.

---

### E39 — Cible `wiring` introuvable

```
fichier.oc:3:5: error: interface 'Repo': 'wiring PostgresRepo' target class not found
```

La classe visée par un `wiring` n'existe nulle part dans le programme (ni comme classe, ni — cas signalé séparément avec un message dédié — comme `generic` nu, ambigu tant qu'il n'est pas instancié).

```ocara
interface Repo {
    method save(): void
    wiring DoesNotExist   // ❌ E39 — aucune classe "DoesNotExist" dans le programme
}
```

**Note :** un `wiring` vers une classe **réellement** inexistante (aucun fichier ni symbole nulle part) est le plus souvent intercepté **avant** ce diagnostic, par l'échec standard de chargement d'import (`wiring` agit comme un import implicite, voir §17.1) — même comportement qu'un `import a.b.NomInexistant` ordinaire, pas une régression. Ce diagnostic E39 couvre le cas où le nom résout bien vers **un fichier existant**, mais que ce fichier ne contient aucune **classe** de ce nom (par exemple une fonction du même nom).

**Correction :** corriger le nom de la classe visée par `wiring`, ou créer la classe manquante.

---

### E40 — Cible `wiring` n'implémente pas l'interface

```
fichier.oc:3:5: error: interface 'Repo': 'wiring PostgresRepo' target class 'PostgresRepo' does not 'implements Repo'
```

La classe visée par un `wiring` existe bien, mais ne déclare pas `implements <CetteInterface>` — la vérification complète de compatibilité de signature (E09 : arité, staticité, types des paramètres et du retour) ne s'exécute d'ailleurs QUE pour les classes qui `implements` réellement l'interface visée ; sans ce diagnostic, une classe `wiring`mais incompatible ne serait jamais signalée avant de produire un mauvais résultat à l'exécution.

```ocara
interface Repo {
    method save(): void
    wiring PostgresRepo
}

class PostgresRepo {          // ❌ E40 — ne déclare pas `implements Repo`
    public method save(): void {
    }
}
```

**Correction :** ajouter `implements Repo` à la classe visée (et s'assurer que sa signature correspond réellement à l'interface, sous peine de E09 ensuite).

---

### E41 — Alias d'import ne correspondant à aucun `wiring`

```
fichier.oc:1:1: error: alias 'NotAWiringTarget' does not match any `wiring` of interface 'Repo' (available: PostgresRepo, InMemoryRepo)
```

`import Interface as Alias` où `Interface` déclare au moins un `wiring`, mais `Alias` ne correspond au nom simple (dernier segment du chemin pointé) d'**aucun** de ses `wiring` — voir §17.1 de l'EBNF. Un alias sur une interface `wiring`n'est jamais un simple renommage cosmétique : il doit désigner sans ambiguïté l'une des implémentations concrètes déclarées.

```ocara
// configs/Repo.oc : interface Repo { wiring PostgresRepo  wiring InMemoryRepo }
import configs.Repo as NotAWiringTarget   // ❌ E41 — ne correspond à aucun wiring de Repo
```

**Correction :** utiliser comme alias le nom simple exact d'un des `wiring` déclarés par l'interface (voir la liste "available" dans le message d'erreur), ou importer l'interface sans alias pour résoudre vers le premier `wiring` déclaré.

---

### E42 — Deux `wiring` de la même interface partageant le même nom simple

```
fichier.oc:1:1: error: interface 'Repo' declares two 'wiring' targets with the same simple name 'PostgresRepo' (3:5 and 4:5) — alias resolution could not tell them apart
```

Deux `wiring` d'une même interface pointent vers des chemins différents dont le **dernier segment** (nom simple) est identique — la résolution d'alias (`import Interface as X`) ne pourrait alors plus savoir, à partir du seul nom simple `X`, laquelle des deux cibles est visée. Signalé sur la déclaration de l'**interface** elle-même, avec les positions des deux `wiring` en cause.

```ocara
interface Repo {
    method save(): void
    wiring infra.a.PostgresRepo
    wiring infra.b.PostgresRepo   // ❌ E42 — même nom simple "PostgresRepo" que le wiring précédent
}
```

**Correction :** renommer l'une des deux classes concrètes (ou son alias d'import côté fichier source), afin que chaque `wiring` d'une même interface ait un nom simple distinct.

---

### E43 — `resolve` sur une expression qui n'est pas `Resolvable<T>`

```
fichier.oc:1:1: error: 'resolve' expects a 'Resolvable<T>' expression (the result of calling an 'async' function/method), found 'string'
```

`resolve expr` attend que `expr` soit de type `Resolvable<T>` (le handle produit par l'appel d'une fonction/méthode `async` — voir §14.5 de l'EBNF et docs/roadmap.d/langage-async-non-int-return-type-check.md). Avant ce diagnostic, une expression qui n'était pas issue (directement ou par indirection non suivie) d'un appel `async` retombait silencieusement sur `Type::Int` ; `Resolvable<T>` porte maintenant l'information nécessaire dans le type lui-même, donc toute autre expression est désormais une vraie erreur de type.

```ocara
function main(): int {
    var s:string = "hello"
    var r:string = resolve s   // ❌ E43 — 's' n'est pas 'Resolvable<T>'
    return 0
}
```

**Correction :** n'appliquer `resolve` qu'à une expression de type `Resolvable<T>` — le résultat direct ou stocké d'un appel à une fonction/méthode `async`.

---

### E44 — Type de retour déclaré d'une fonction/méthode `async` lui-même `Resolvable<T>`

```
fichier.oc:1:1: error: 'fetch' is 'async' and declares 'Resolvable<T>' as its own return type — an 'async' function/method already wraps its declared return type in 'Resolvable<T>' automatically at the call site; declare the real return type here instead (e.g. 'string', not 'Resolvable<string>')
```

Une fonction/méthode `async` emballe déjà automatiquement son type de retour **déclaré** dans `Resolvable<T>` au site d'appel (voir §14.5 de l'EBNF) — déclarer `Resolvable<T>` comme type de retour de la fonction/méthode `async` elle-même produirait un double emballage implicite `Resolvable<Resolvable<T>>` absurde.

```ocara
class Doubler {
    public static async method fetch(): Resolvable<string> {   // ❌ E44
        return "hi"
    }
}
```

**Correction :** déclarer le VRAI type de retour (`string`), jamais `Resolvable<...>` — la substitution vers `Resolvable<T>` est appliquée automatiquement à chaque site d'appel.

---

### E45 — Appel mélangeant arguments positionnels et nommés

```
fichier.oc:1:1: error: call to 'box' mixes positional and named arguments — a call is either fully positional or fully named
```

Un appel est soit entièrement positionnel, soit entièrement nommé (voir §14.6 de l'EBNF et docs/roadmap.d/langage-named-arguments.md) — jamais un mélange des deux.

```ocara
function box(text:string, left:string = "<", right:string = ">"): string { return `${left}${text}${right}` }

box("x", right: "|")          // ❌ E45
```

**Correction :** tout nommer (`box(text: "x", right: "|")`) ou tout passer en position (`box("x", "<", "|")`).

---

### E46 — Nom d'argument inconnu

```
fichier.oc:1:1: error: 'String::replace' has no parameter named 'subject' — valid names: s, from, to
```

Le nom ne correspond à aucun paramètre de la cible résolue ; le message liste les noms valides (paramètres non variadics, dans l'ordre de déclaration). Pour un builtin, ce sont les noms documentés dans `docs/builtins/*.md`.

```ocara
String::replace(subject: "aXb", search: "X", replace: "-")   // ❌ E46
```

**Correction :** utiliser l'un des noms listés — `String::replace(s: "aXb", from: "X", to: "-")`.

---

### E47 — Argument nommé fourni deux fois

```
fichier.oc:1:1: error: argument 'a' is provided twice in this call to 'pair'
```

```ocara
function pair(a:int, b:int): int { return a - b }

pair(a: 1, a: 2)   // ❌ E47
```

**Correction :** ne fournir chaque paramètre qu'une seule fois.

---

### E48 — Paramètre variadic passé par son nom

```
fichier.oc:1:1: error: parameter 'nums' of 'sum' is variadic and can only be passed positionally
```

Un paramètre `variadic<T>` reste positionnel uniquement (comme il ne peut pas avoir de valeur par défaut, §14.1) — et un appel ne mélangeant jamais positionnel et nommé (E45), un appel à une fonction variadic dont on veut fournir le variadic est entièrement positionnel.

```ocara
function sum(label:string, nums:variadic<int>): string { return label }

sum(label: "x", nums: 1)   // ❌ E48
sum("x", 1, 2, 3)          // ✅
```

---

### E49 — Paramètre obligatoire absent d'un appel nommé

```
fichier.oc:1:1: error: missing argument 'a' in this named call to 'pair' — it has no default value
```

Dans un appel nommé, tout paramètre **sans valeur par défaut** doit être fourni ; seuls les paramètres avec valeur par défaut peuvent être omis (y compris au milieu de la liste). Pour un builtin, un paramètre optionnel ne peut être omis qu'en fin de liste.

```ocara
pair(b: 2)   // ❌ E49 — 'a' manque
```

**Correction :** fournir le paramètre manquant (`pair(a: 1, b: 2)`), ou lui donner une valeur par défaut dans la déclaration.

---

### E50 — Argument nommé sur une cible aux noms de paramètres inconnus

```
fichier.oc:1:1: error: named argument 'a' cannot be used here: the parameter names of the called target are not statically known (e.g. a call through a 'Function<...>' value) — pass the arguments positionally
```

Les noms de paramètres n'existent pas pour toutes les cibles : une valeur de type `Function<T(...)>` (§14.3) ne référence que les **types** de ses paramètres ; une classe opaque (import non résolu) ou un receveur de type `mixed` n'a pas de signature connue. Limite assumée, pas un oubli.

```ocara
function pair(a:int, b:int): int { return a - b }

var f:Function<int(int, int)> = pair
f(a: 1, b: 2)    // ❌ E50
f(1, 2)          // ✅
```

Variante émise après l'analyse sémantique, pour un appel nommé dans le corps d'un `generic` (non parcouru par l'analyse sémantique) dont la cible dépend du type d'un receveur autre que `self` : `named argument 'x' cannot be resolved here: this call's target depends on a type not known outside semantic analysis (...)`.

**Correction :** passer les arguments en position.

---

### E51 — Champ `private` dans un `struct`

```
fichier.oc:1:1: error: 'private' is not allowed in struct 'UserDTO' — a struct is a transparent data aggregate with no invariant to protect; use 'protected' (visible to extending structs) or a class
```

Un `struct` (§16.7 de l'EBNF) est un agrégat de données transparent : `private` sert à protéger un invariant qu'une classe maintient elle-même via ses méthodes, ce qu'un struct (sans méthode) ne fait jamais. Vouloir un champ privé est le signal qu'on veut en réalité une `class`.

```ocara
struct UserDTO {
    private token:string   // ❌ E51
}
```

**Correction :** retirer `private` (public par défaut), utiliser `protected` pour un champ réservé aux structs dérivés, ou transformer le struct en `class`.

---

### E52 — `extends` entre `struct` et `class`

```
fichier.oc:1:1: error: struct 'P' cannot extend class 'C' — a struct can only extend another struct
fichier.oc:1:1: error: class 'C' cannot extend struct 'P' — a struct can only be extended by another struct
```

Un struct n'étend qu'un struct et n'est étendu que par un struct : une classe dérivée ajouterait des méthodes/un `init` à un agrégat dont le constructeur est généré depuis ses champs, et un struct dérivé d'une classe hériterait de méthodes qu'il ne peut pas déclarer lui-même.

```ocara
class Base { }
struct P extends Base { x:int }   // ❌ E52
```

**Correction :** faire hériter un struct d'un struct, ou transformer les deux en classes.

---

### E53 — Champ hérité redéclaré par un `struct` dérivé

```
fichier.oc:2:22: error: field 'x' of struct 'Q' is already declared by a parent struct (line 1) — a struct cannot redeclare an inherited field
```

Le constructeur généré d'un struct dérivé reprend les champs de tous ses parents, puis les siens : un même nom apparaîtrait deux fois parmi ses paramètres (et deux fois dans l'objet).

```ocara
struct P { x:int }
struct Q extends P { x:int }   // ❌ E53
```

**Correction :** renommer le champ du struct dérivé, ou le supprimer (il est déjà hérité).

---

### E54 — Champ `private`/`protected` inaccessible

```
fichier.oc:9:18: error: field 'y' of 'C' is protected — it is only accessible from 'C' and the classes/structs that extend it
fichier.oc:9:25: error: field 'z' of 'C' is private — it is only accessible from inside 'C' (expose it through a public method)
```

Un champ `private` n'est accessible que depuis la classe qui le déclare, un champ `protected` que depuis cette classe et ses descendantes (§16.3 de l'EBNF) — en lecture comme en affectation (`=`, `++`/`--`). Jusqu'à ce diagnostic, ces mots-clés étaient silencieusement sans effet : un champ privé se lisait et s'écrivait depuis n'importe où.

```ocara
class C {
    protected property y:int
    private property z:int
    init() {
        self.y = 2
        self.z = 3
    }
}

var c:C = use C()
IO::writeln(`${c.y}`)   // ❌ E54 — protected
c.z = 4                 // ❌ E54 — private
```

**Correction :** exposer la donnée via une méthode publique de la classe, ou rendre le champ `public` s'il fait réellement partie de l'interface de la classe.

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
