# ocara.UnitTest

> Classe builtin pour l'écriture de tests unitaires en Ocara.  
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

Vérifie que `a > b`.

```ocara
UnitTest::assertGreater(score, 50)
```

---

#### `UnitTest::assertLess(a, b)`

Vérifie que `a < b`.

```ocara
UnitTest::assertLess(errors, 1)
```

---

#### `UnitTest::assertGreaterOrEquals(a, b)`

Vérifie que `a >= b`.

```ocara
UnitTest::assertGreaterOrEquals(score, 60)
```

---

#### `UnitTest::assertLessOrEquals(a, b)`

Vérifie que `a <= b`.

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

## Format de sortie

Chaque assertion écrit une ligne sur stdout :

```
PASS assertEquals: 42 == 42
FAIL assertEquals: attendu "alice" mais obtenu "bob"
PASS assertTrue
FAIL assertGreater: 10 n'est pas > 50
```

Ce format est lu par `ocaraunit` pour produire le rapport final.

---

## Exemple complet

Fichier `MathTest.oc` :

```ocara
import ocara.UnitTest

function add(a: int, b: int): int {
    return a + b
}

function addBasicTest(): int {
    UnitTest::assertEquals(5, add(2, 3))
    UnitTest::assertEquals(0, add(-1, 1))
    return 0
}

function addComparisonTest(): int {
    UnitTest::assertTrue(add(1, 1) > 0)
    UnitTest::assertNotEquals(99, add(2, 3))
    UnitTest::assertGreater(add(10, 5), 10)
    return 0
}
```

Sortie d'`ocaraunit` lors de l'exécution de `addBasicTest` :

```
PASS assertEquals: 5 == 5
PASS assertEquals: 0 == 0
```

---

## Conventions pour ocaraunit

Les fichiers de test sont nommés `*Test.oc` (ex : `MathTest.oc`, `UserServiceTest.oc`).  
Un fichier de test peut être un **script simple** ou une **classe**.

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

Voir `docs/tools/ocaraunit.md` pour la documentation du runner.
