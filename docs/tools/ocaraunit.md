# ocaraunit

> Runner de tests unitaires pour Ocara.  
> Découvre, compile et exécute les fichiers `*Test.oc`, puis rapporte les résultats et la couverture.

---

## Installation

```bash
make build-tools
make install-tools   # installe dans /usr/local/bin/ocaraunit
```

---

## Usage

```
ocaraunit [--coverage [<dossier>]]
```

| Argument                  | Description                                                        |
|---------------------------|--------------------------------------------------------------------|
| `--coverage [<dossier>]`  | Active l'analyse de couverture sur `<dossier>` (défaut : `.`)      |

`ocaraunit` cherche toujours les fichiers de test dans le dossier **`tests/`** à la racine du répertoire de lancement. Si `tests/` n'existe pas, `ocaraunit` avertit et quitte.

**Exemples :**

```bash
ocaraunit                          # lance les tests dans tests/
ocaraunit --coverage               # tests + couverture sur .
ocaraunit --coverage src/          # tests + couverture sur src/
ocaraunit --coverage examples/     # tests + couverture sur examples/
```

Via Makefile :

```bash
make unittest                      # lance les tests dans tests/
```

---

## Convention de découverte

### Dossier `tests/`

`ocaraunit` cherche **toujours** les fichiers de test dans un dossier `tests/` à la racine du projet (là où la commande est lancée).

```
projet/
├── tests/          ← les fichiers *Test.oc sont ici
├── src/
└── examples/
```

Si le dossier `tests/` n'existe pas, `ocaraunit` affiche un avertissement et quitte.

### Nommage des fichiers de test

Le nom du fichier de test correspond au nom du fichier source suffixe de `Test` :

| Fichier source              | Exemple de fichier de test              |
|-----------------------------|------------------------------------------|
| `src/Math.oc`               | `tests/MathTest.oc`                      |
| `examples/01_variables.oc`  | `tests/01_variablesTest.oc`              |
| `services/UserService.oc`   | `tests/services/UserServiceTest.oc`      |

Les fichiers de test peuvent être organisés **en sous-dossiers** dans `tests/`. `ocaraunit` les découvre récursivement.

### Fichiers de test

Dans un fichier sans déclaration de classe, toute fonction dont le nom se termine par `Test` est un test :

```ocara
import ocara.UnitTest

function add(a: int, b: int): int {
    return a + b
}

function addTest(): int {
    UnitTest::assertEquals(5, add(2, 3))
    UnitTest::assertEquals(0, add(-1, 1))
    return 0
}

function negativeAddTest(): int {
    UnitTest::assertLess(add(-5, -3), 0)
    return 0
}

// Pas de suffixe Test → ignorée par ocaraunit
function helper(): int {
    return 42
}
```

### Méthodes de test — classe

Dans un fichier avec une déclaration de classe, seules les méthodes **publiques** dont le nom se termine par `Test` sont des tests.

Les méthodes **statiques** (`public static method fooTest`) sont également reconnues.

```ocara
import ocara.UnitTest

class MathTest {
    public method addTest(): int {
        UnitTest::assertEquals(5, add(2, 3))
        return 0
    }

    public method negativeAddTest(): int {
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

## Format de sortie des tests

```
══════════════════════════════════════════════
 Tests ocaraunit
══════════════════════════════════════════════

src/MathTest.oc:
  PASS assertEquals: 5 == 5
  PASS assertEquals: 0 == 0
  PASS assertLess: -8 < 0
  3 PASS  0 FAIL

src/UserTest.oc:
  PASS assertEquals: alice == alice
  FAIL assertEquals: attendu 42 mais obtenu 0
  2 PASS  1 FAIL

══════════════════════════════════════════════
Résultat global : 5 PASS  1 FAIL  0 ERREUR(S)
══════════════════════════════════════════════
```

- `PASS` en vert, `FAIL` en rouge
- Résultat global en vert si tout est OK, rouge sinon
- Code de sortie `0` si tous les tests passent, `1` sinon

---

## Analyse de couverture (`--coverage`)

Après l'exécution des tests, `ocaraunit` analyse les fichiers `.oc` non-test du projet et calcule le taux de couverture par fichier.

**Règle de couverture :** une fonction ou méthode `foo` est considérée couverte s'il existe un test nommé `fooTest` dans les fichiers `*Test.oc`.

Sont analysées : `function foo`, `public method foo`, `protected method foo`, `private method foo`, `public static method foo` (et variantes `protected`/`private static`).

### Affichage

```
══════════════════════════════════════════════
 Couverture de tests
══════════════════════════════════════════════

  src/Math.oc          100.0%  [████████████████████]  3/3 fonctions
  src/UserService.oc    75.0%  [███████████████░░░░░]  3/4 fonctions
  src/Config.oc          0.0%  [░░░░░░░░░░░░░░░░░░░░]  0/2 fonctions

══════════════════════════════════════════════
 Couverture globale : 66.7% (6/9 fonctions)
══════════════════════════════════════════════
```

### Règles de couleur

| Couverture          | Couleur  |
|---------------------|----------|
| 100 %               | Vert     |
| ≥ 75 % et < 100 %   | Jaune    |
| < 75 %              | Rouge    |

La même règle s'applique au pourcentage global en bas.

---

## Configuration `.ocaraunit`

Créez un fichier `.ocaraunit` à la racine de votre projet pour configurer ocaraunit.  
`ocaraunit` remonte l'arborescence depuis le dossier analysé pour le trouver.

### Format

```ini
# .ocaraunit

[exclude]
# Exclure des fichiers ou répertoires de l'analyse de couverture
# (les fichiers de test *Test.oc dans ces chemins sont également ignorés)
vendor/
examples/
generated/
src/legacy/Config.oc
```

### Exemple complet

```ini
# .ocaraunit — configuration ocaraunit

[exclude]
vendor/
examples/
```

---

## Variable d'environnement

| Variable | Description                                     |
|----------|-------------------------------------------------|
| `OCARA`  | Chemin vers le binaire `ocara` (défaut : auto-détecté) |

Auto-détection dans l'ordre :
1. Variable `OCARA`
2. `./target/release/ocara`
3. `../target/release/ocara`
4. `../../target/release/ocara`
5. `ocara` dans le `PATH`

---

## Codes de sortie

| Code | Signification                             |
|------|-------------------------------------------|
| `0`  | Tous les tests ont réussi                 |
| `1`  | Au moins un test a échoué ou erreur       |

---

## Intégration Makefile

```makefile
unittest: build-tools build
	OCARA=./target/release/ocara ./target/release/ocaraunit $(dossier)

unittest-coverage: build-tools build
	OCARA=./target/release/ocara ./target/release/ocaraunit --coverage $(dossier)
```

---

## Projet exemple

Le dépôt Ocara fournit un projet d'exemple complet dans `examples/project/` :

```
examples/project/
├── .ocaraunit               ← (optionnel) configuration
├── main.oc                  ← classes Color (statiques), Score, Student + fonctions libres
├── classes/
│   ├── Models.oc
│   ├── Services.oc
│   └── Utils.oc
└── tests/
    ├── mainTest.oc          ← couvre Color, Score, Student
    ├── ModelsTest.oc
    ├── ServicesTest.oc
    └── UtilsTest.oc
```

Pour le lancer :

```bash
cd examples/project
OCARA=../../target/release/ocara ../../target/release/ocaraunit --coverage
```

Résultat attendu :

```
══════════════════════════════════════════════
 Tests ocaraunit
══════════════════════════════════════════════

tests/ModelsTest.oc:   4 PASS  0 FAIL
tests/ServicesTest.oc: 8 PASS  0 FAIL
tests/UtilsTest.oc:   10 PASS  0 FAIL
tests/mainTest.oc:    27 PASS  0 FAIL

══════════════════════════════════════════════
Résultat global : 49 PASS  0 FAIL  0 ERREUR(S)
══════════════════════════════════════════════

══════════════════════════════════════════════
 Couverture de tests
══════════════════════════════════════════════

  classes/Models.oc      100.0%  [████████████████████]  3/3 fonctions
  classes/Services.oc    100.0%  [████████████████████]  5/5 fonctions
  classes/Utils.oc       100.0%  [████████████████████]  3/3 fonctions
  main.oc                100.0%  [████████████████████]  12/12 fonctions

══════════════════════════════════════════════
 Couverture globale : 100.0% (23/23 fonctions)
══════════════════════════════════════════════
```

---

## Voir aussi

- [docs/builtins/UnitTest.md](../builtins/UnitTest.md) — assertions disponibles
- [docs/tools/ocaracs.md](ocaracs.md) — analyseur de style
