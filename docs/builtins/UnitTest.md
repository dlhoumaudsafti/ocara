# ocara.UnitTest

> Classe de la bibliothèque runtime pour l'écriture de tests unitaires en Ocara.  
> Utilisée conjointement avec l'outil `ocaraunit`.

---

## Import

```ocara
import ocara.UnitTest
```

---

## Méthodes statiques

### Égalité

#### `UnitTest::assertEquals(expected, actual)`

Vérifie que `expected` est égal à `actual`.

```ocara
UnitTest::assertEquals(42, result)
UnitTest::assertEquals("bonjour", message)
```

---

#### `UnitTest::assertNotEquals(expected, actual)`

Vérifie que `expected` est différent de `actual`.

```ocara
UnitTest::assertNotEquals(0, count)
```

---

### Booléens

#### `UnitTest::assertTrue(value)`

Vérifie que `value` est vraie (non nulle).

```ocara
UnitTest::assertTrue(user.isActive())
UnitTest::assertTrue(list.contains("alice"))
```

---

#### `UnitTest::assertFalse(value)`

Vérifie que `value` est fausse (nulle ou zéro).

```ocara
UnitTest::assertFalse(list.isEmpty())
```

---

### Nullité

#### `UnitTest::assertNull(value)`

Vérifie que `value` est nulle (0).

```ocara
UnitTest::assertNull(result)
```

---

#### `UnitTest::assertNotNull(value)`

Vérifie que `value` est non nulle.

```ocara
UnitTest::assertNotNull(user)
```

---

### Comparaisons numériques

#### `UnitTest::assertGreater(a, b)`

Vérifie que `a greater b`.

```ocara
UnitTest::assertGreater(score, 50)
```

---

#### `UnitTest::assertLess(a, b)`

Vérifie que `a smaller b`.

```ocara
UnitTest::assertLess(errors, 1)
```

---

#### `UnitTest::assertGreaterOrEquals(a, b)`

Vérifie que `a greater or equal b`.

```ocara
UnitTest::assertGreaterOrEquals(score, 60)
```

---

#### `UnitTest::assertLessOrEquals(a, b)`

Vérifie que `a smaller or equal b`.

```ocara
UnitTest::assertLessOrEquals(retries, 3)
```

---

### Chaînes

#### `UnitTest::assertContains(haystack, needle)`

Vérifie que la chaîne `haystack` contient `needle`.

```ocara
UnitTest::assertContains("bonjour monde", "monde")
```

---

### Vide / non-vide

#### `UnitTest::assertEmpty(value)`

Vérifie que la valeur est vide (chaîne vide ou nulle).

```ocara
UnitTest::assertEmpty(errors)
```

---

#### `UnitTest::assertNotEmpty(value)`

Vérifie que la valeur est non vide.

```ocara
UnitTest::assertNotEmpty(results)
```

---

### Manuel

#### `UnitTest::pass(message)`

Force un succès avec un message personnalisé.

```ocara
UnitTest::pass("cas non applicable sur cette plateforme")
```

---

#### `UnitTest::fail(message)`

Force un échec avec un message personnalisé.

```ocara
UnitTest::fail("cette branche ne devrait jamais être atteinte")
```

---

### Types

Ces assertions vérifient la **nature runtime** de la valeur (paramètre `value:mixed`). Elles affichent `PASS <nom>` en cas de succès.

#### `UnitTest::assertArray(value)`

Vérifie que `value` est un tableau (`array<T>`). Code 119 sinon.

```ocara
UnitTest::assertArray([1, 2, 3])
```

---

#### `UnitTest::assertMap(value)`

Vérifie que `value` est une map (`map<K, V>`). Code 118 sinon.

```ocara
const m:map<string, int> = {"a": 1}
UnitTest::assertMap(m)
```

---

#### `UnitTest::assertFunction(value)`

Vérifie que `value` est une fonction (closure `nameless` ou référence de fonction). Code 115 sinon.

```ocara
UnitTest::assertFunction(nameless(x:int): int { return x })
```

---

#### `UnitTest::assertClass(value)`

Vérifie que `value` est une instance de classe. Code 116 sinon.

```ocara
scoped p:Point = use Point(1)
UnitTest::assertClass(p)
```

---

#### `UnitTest::assertEnum(value)`

Vérifie que `value` est un objet (même test runtime que `assertClass`). Code 117 sinon.

> Les variantes d'enum sont des entiers à l'exécution : `assertEnum` est vérifié **à la compilation**. L'argument doit être une variante (`UnitTest::assertEnum(Color::RED)`) ou une valeur de type enum, sinon erreur de compilation (« expected type 'enum' »).

---

### Exceptions

#### `UnitTest::assertRaises(callable) → Exception`

Exécute `callable` (closure sans paramètre) dans un `try` interne. Si une exception est levée, affiche `PASS assertRaises` et **retourne l'exception capturée** (type déclaré `mixed`, assignable à une variable `Exception`) pour l'inspecter ensuite. Si `callable` se termine sans lever, lève une `UnitTestException` (code 120, `assertRaises: callable did not raise an exception`).

```ocara
const m:map<string, int> = {"a": 1}
const e:Exception = UnitTest::assertRaises(nameless(): void {
    Map::get(m, "absent")
})
UnitTest::assertExceptionCodeEquals(e.code, 101)
```

---

#### `UnitTest::assertExceptionMessageEquals(message:string, expected:string)`
#### `UnitTest::assertExceptionMessageNotEquals(message:string, expected:string)`

Compare `e.message` à la chaîne attendue (égalité exacte). Codes 121 / 122 en cas d'échec.

```ocara
UnitTest::assertExceptionMessageEquals(e.message, "Key not found: absent")
UnitTest::assertExceptionMessageNotEquals(e.message, "")
```

---

#### `UnitTest::assertExceptionCodeEquals(code:int, expected:int)`
#### `UnitTest::assertExceptionCodeNotEquals(code:int, expected:int)`

Compare `e.code` à l'entier attendu. Codes 123 / 124 en cas d'échec.

```ocara
UnitTest::assertExceptionCodeEquals(e.code, 101)
UnitTest::assertExceptionCodeNotEquals(e.code, 0)
```

---

#### `UnitTest::assertExceptionSourceEquals(source:string, expected:string)`
#### `UnitTest::assertExceptionSourceNotEquals(source:string, expected:string)`

Compare `e.source` (classe builtin à l'origine de l'exception, ex. `"Map"`) à la chaîne attendue. Codes 125 / 126 en cas d'échec.

```ocara
UnitTest::assertExceptionSourceEquals(e.source, "Map")
UnitTest::assertExceptionSourceNotEquals(e.source, "File")
```

Sortie de l'exemple ci-dessus :

```
PASS assertRaises
PASS assertExceptionCodeEquals: 101 == 101
PASS assertExceptionMessageEquals: "Key not found: absent" == "Key not found: absent"
PASS assertExceptionSourceEquals: "Map" == "Map"
```

Voir [Exception](Exception.md) pour les champs `message`, `code` et `source`.

---

## Format de sortie

Chaque assertion réussie écrit une ligne `PASS` sur stdout :

```
PASS assertEquals: 42 == 42
PASS assertTrue
```

Une assertion en échec lève une `UnitTestException` (message en anglais, par exemple `assertGreater: 10 is not > 50`) : l'exécution du fichier de test s'arrête là, et `ocaraunit` le compte comme **erreur d'exécution** en affichant le message. Les assertions déjà passées restent comptées.

```
  PASS assertEquals: 42 == 42
  PASS assertTrue
  2 PASS  0 FAIL (compile: 937ms, run: 1ms)
  ERREUR exécution :
  … Exception raised: UnitTestException …
  Message: assertGreater: 10 is not > 50
```

`ocaraunit` lit ces lignes pour produire le rapport final.

---

## Exemple complet

Un projet d'exemple complet est disponible dans `examples/project/` du dépôt Ocara.
Il illustre toutes les conventions décrites ci-dessous.

### Structure

```
examples/project/
├── main.oc               ← classes Color, Score, Student + fonctions libres
├── classes/
│   ├── Models.oc
│   ├── Services.oc
│   └── Utils.oc
└── tests/
    ├── mainTest.oc       ← teste Color (statiques), Score, Student
    ├── ModelsTest.oc
    ├── ServicesTest.oc
    └── UtilsTest.oc
```

### Exemple — tester des méthodes statiques (`Color`)

`main.oc` déclare :

```ocara
class Color {
    public const RED:string = "rouge"
    // ...
    public static method mix(a:string, b:string): string { ... }
    public static method is_primary(c:string): bool { ... }
    public static method describe(c:string): string { ... }
}
```

`tests/mainTest.oc` les couvre :

```ocara
import ocara.UnitTest
import main

class mainTest {

    public method mixTest(): int {
        UnitTest::assertNotNull(Color::mix(Color::RED, Color::BLUE))
        UnitTest::pass("mix appelé sans erreur")
        return 0
    }

    public method is_primaryTest(): int {
        UnitTest::assertTrue(Color::is_primary(Color::RED))
        UnitTest::assertFalse(Color::is_primary("violet"))
        return 0
    }

    public method describeTest(): int {
        UnitTest::assertNotNull(Color::describe(Color::RED))
        UnitTest::pass("describe appelé sans erreur")
        return 0
    }
}
```

### Exemple — tester des méthodes d'instance

```ocara
    public method compareTest(): int {
        scoped s:Score = use Score("Alice", 80)
        UnitTest::assertEquals(1,  s.compare(70))
        UnitTest::assertEquals(-1, s.compare(90))
        UnitTest::assertEquals(0,  s.compare(80))
        return 0
    }
```

Sortie d'`ocaraunit` :

```
PASS assertEquals: 1 == 1
PASS assertEquals: -1 == -1
PASS assertEquals: 0 == 0
```

---

## Conventions pour ocaraunit

Les fichiers de test sont placés dans un dossier **`tests/`** à la racine du projet.  
Un fichier de test peut être un **script simple** ou une **classe**.

### Nommage

Le nom du fichier de test correspond au nom du fichier source suffixe de `Test` :

| Fichier source              | Exemple de fichier de test              |
|-----------------------------|------------------------------------------|
| `src/Math.oc`               | `tests/MathTest.oc`                      |
| `examples/01_variables.oc`  | `tests/01_variablesTest.oc`              |
| `services/UserService.oc`   | `tests/services/UserServiceTest.oc`      |

Les fichiers de test peuvent être organisés en sous-dossiers dans `tests/`. `ocaraunit` les découvre récursivement.

### Script sans classe

Toutes les fonctions dont le nom se termine par `Test` sont exécutées par `ocaraunit`.  
Les fonctions globales n'ont pas de modificateur de visibilité en Ocara.

```ocara
import ocara.UnitTest

function addTest(): int {
    UnitTest::assertEquals(5, add(2, 3))
    return 0
}

function negativeNumbersTest(): int {
    UnitTest::assertLess(add(-5, -3), 0)
    return 0
}

// Pas de suffixe Test → ignorée par ocaraunit
function helper(): int {
    return 42
}
```

### Classe de test

Seules les méthodes **publiques** dont le nom se termine par `Test` sont exécutées par `ocaraunit`.  
Les méthodes de classe utilisent le mot-clé `method` précédé d'un modificateur de visibilité.

Les méthodes **statiques** (`public static method fooTest`) sont aussi reconnues et exécutées.

```ocara
import ocara.UnitTest

class MathTest {
    public method addTest(): int {
        UnitTest::assertEquals(5, add(2, 3))
        return 0
    }

    public method negativeNumbersTest(): int {
        UnitTest::assertLess(add(-5, -3), 0)
        return 0
    }

    // private → ignorée par ocaraunit
    private method helper(): int {
        return 42
    }
}
```

---

## Exceptions

### UnitTestException

**Toutes les assertions** lèvent une **UnitTestException** lorsqu'elles échouent.

**Codes d'erreur** :

| Code | Méthode | Condition d'échec |
|------|---------|-------------------|
| 101  | `assertEquals` | Valeurs différentes |
| 102  | `assertNotEquals` | Valeurs égales |
| 103  | `assertTrue` | Valeur fausse |
| 104  | `assertFalse` | Valeur vraie |
| 105  | `assertNull` | Valeur non nulle |
| 106  | `assertNotNull` | Valeur nulle |
| 107  | `assertGreater` | a ≤ b |
| 108  | `assertLess` | a ≥ b |
| 109  | `assertGreaterOrEquals` | a < b |
| 110  | `assertLessOrEquals` | a > b |
| 111  | `assertContains` | Substring non trouvée |
| 112  | `assertEmpty` | Valeur non vide |
| 113  | `assertNotEmpty` | Valeur vide |
| 114  | `fail()` | Échec explicite |
| 115  | `assertFunction` | Non fonction |
| 116  | `assertClass` | Non instance |
| 117  | `assertEnum` | Non enum |
| 118  | `assertMap` | Non map |
| 119  | `assertArray` | Non array |
| 120  | `assertRaises` | Le callable n'a levé aucune exception |
| 121  | `assertExceptionMessageEquals` | Messages différents |
| 122  | `assertExceptionMessageNotEquals` | Messages égaux |
| 123  | `assertExceptionCodeEquals` | Codes différents |
| 124  | `assertExceptionCodeNotEquals` | Codes égaux |
| 125  | `assertExceptionSourceEquals` | Sources différentes |
| 126  | `assertExceptionSourceNotEquals` | Sources égales |

**Exemples d'utilisation** :

```ocara
import ocara.UnitTest
import ocara.UnitTestException
import ocara.IO

// Exemple 1 : Capture d'une assertion qui échoue
function testWithException(): int {
    try {
        UnitTest::assertEquals(42, 10)
        IO::writeln("✗ Should have thrown")
    } on e is UnitTestException {
        IO::writeln(`✓ Exception: ${e.message}`)
        IO::writeln(`  Code: ${e.code}`)
    }
    return 0
}

// Exemple 2 : Vérifier plusieurs assertions
function testMultiple(): int {
    var passed:int = 0
    var failed:int = 0
    
    try {
        UnitTest::assertTrue(1 equal 1)
        passed = passed + 1
    } on e is UnitTestException {
        failed = failed + 1
    }
    
    try {
        UnitTest::assertEquals("hello", "world")
    } on e is UnitTestException {
        IO::writeln(`Échec attendu: ${e.message}`)
        failed = failed + 1
    }
    
    IO::writeln(`Tests: ${passed} réussis, ${failed} échoués`)
    return 0
}

// Exemple 3 : Test conditionnel avec catch
function testSafe(): int {
    try {
        var result:int = compute()
        UnitTest::assertGreater(result, 0)
        IO::writeln("✓ Test passed")
    } on e is UnitTestException {
        IO::writeln(`✗ Test failed: ${e.message}`)
        return 1
    }
    return 0
}

// Exemple 4 : Échec explicite avec fail()
function testFailure(): int {
    try {
        if some_condition() {
            UnitTest::fail("Condition invalide détectée")
        }
    } on e is UnitTestException {
        if e.code equal 114 {
            IO::writeln("✓ Échec explicite capturé")
        }
    }
    return 0
}

// Exemple 5 : Catch générique
function testGeneric(): int {
    try {
        UnitTest::assertNotNull(getValue())
    } on e {
        IO::writeln(`Exception: ${e.message}`)
        IO::writeln(`Source: ${e.source}`)
    }
    return 0
}
```

**Notes** :

- Les exceptions permettent de **capturer** les échecs d'assertions plutôt que de terminer le test immédiatement
- En l'absence de `try/on`, une UnitTestException non capturée arrête l'exécution du programme
- L'outil `ocaraunit` capture automatiquement les exceptions pour afficher les résultats des tests
- Le champ `e.message` contient une description détaillée de l'échec (valeurs attendues vs réelles)
- Le champ `e.code` permet de distinguer précisément quelle assertion a échoué
- La méthode `pass()` ne lève jamais d'exception, elle affiche seulement "PASS"

---

Voir `docs/tools/ocaraunit.md` pour la documentation du runner.
