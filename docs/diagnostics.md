# Ocara — Diagnostics : erreurs et avertissements

> Référence complète des messages produits par le compilateur `ocara`.

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

---

## Erreurs lexicales

Produites lors de la tokenisation du source.

| Message | Cause | Exemple |
|---------|-------|---------|
| `caractère inattendu 'X'` | Caractère non reconnu par la grammaire | `var x@ = 1` |
| `chaîne non fermée` | Guillemet ouvrant sans guillemet fermant | `var s = "bonjour` |
| `séquence d'échappement invalide '\X'` | `\X` inconnu dans une chaîne | `"\q"` |
| `entier trop grand : N` | Entier littéral dépassant `i64::MAX` | `var n = 99999999999999999999` |

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

---

## Erreurs sémantiques

Produites lors de l'analyse de types et de symboles (`--check` ou compilation).

### E01 — Symbole indéfini

```
fichier.oc:5:10: error: symbole indéfini 'foo'
```

Variable, fonction ou classe utilisée sans avoir été déclarée.

```ocara
var x: int = foo          // 'foo' n'existe pas
```

**Correction :** déclarer `foo` avant utilisation, ou corriger le nom.

---

### E02 — Incompatibilité de types

```
fichier.oc:7:5: error: type attendu 'int', trouvé 'string'
```

La valeur assignée ou retournée ne correspond pas au type déclaré.

```ocara
var n: int = "bonjour"    // string assigné à int
```

**Correction :** utiliser `Convert::str_to_int()` ou corriger le type déclaré.

---

### E03 — Symbole en double

```
fichier.oc:12:5: error: symbole en double 'x'
```

Une variable ou fonction est déclarée deux fois dans le même scope.

```ocara
var x: int = 1
var x: int = 2            // doublon dans le même bloc
```

**Correction :** renommer l'une des deux déclarations.

---

### E04 — Symbole non appelable

```
fichier.oc:8:5: error: 'x' n'est pas appelable
```

Tentative d'appel d'une variable comme si c'était une fonction.

```ocara
var x: int = 42
x()                       // x n'est pas une fonction
```

---

### E05 — Mauvais nombre d'arguments

```
fichier.oc:9:5: error: 'IO::writeln' attend 1 argument(s), 3 fourni(s)
```

Appel d'une fonction avec un nombre d'arguments incorrect.

```ocara
IO::writeln("a", "b", "c")   // writeln n'attend qu'un seul argument
```

**Correction :** consulter la documentation de la fonction dans `docs/builtins/`.

---

### E06 — Type de retour incompatible

```
fichier.oc:15:5: error: retour attendu 'int', trouvé 'string'
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
fichier.oc:20:15: error: 'MaVar' n'est pas une classe
```

Tentative d'instanciation (`new`) d'un symbole qui n'est pas une classe.

---

### E08 — Champ introuvable

```
fichier.oc:22:10: error: champ 'nom' introuvable dans la classe 'Point'
```

Accès à un champ ou méthode inexistant dans une classe.

```ocara
var p: Point = new Point(1, 2)
IO::writeln(p.nom)        // 'nom' n'est pas dans Point
```

---

### E09 — Interface non implémentée

```
fichier.oc:30:1: error: classe 'Cercle' n'implante pas 'Forme::aire' de l'interface 'Forme'
```

Une classe déclare implémenter une interface mais n'en définit pas toutes les méthodes.

**Correction :** ajouter la méthode manquante dans la classe.

---

### E10 — Assignation invalide

```
fichier.oc:35:5: error: impossible d'assigner à 'PI' (immuable ou non-déclaré)
```

Tentative de modification d'une constante (`const`) ou d'un accès statique.

```ocara
Math::PI = 3              // PI est une constante
```

---

### E11 — Méthode non statique appelée statiquement

```
fichier.oc:40:5: error: 'Compte::deposer' n'est pas statique — utilisez une instance
```

Appel d'une méthode d'instance via `Classe::methode` au lieu d'une instance.

```ocara
Compte::deposer(500)      // deposer() n'est pas statique
```

**Correction :** créer une instance : `var c = new Compte(...); c.deposer(500)`.

---

### E12 — Méthode statique appelée sur une instance

```
fichier.oc:45:5: error: 'Math::sqrt' est statique — appelez-la via Math::sqrt sans instance
```

Appel d'une méthode statique via une instance au lieu de la classe directement.

---

## Avertissements sémantiques

Les avertissements ne bloquent pas la compilation mais signalent du code suspect.

### W01 — Variable inutilisée

```
fichier.oc:14:5: warning: variable 'inutile' jamais utilisée
```

Une variable est déclarée mais sa valeur n'est jamais lue.

```ocara
var inutile: string = "jamais lue"   // déclarée mais pas utilisée
```

**Correction :** supprimer la variable ou l'utiliser. Les paramètres de fonctions sont exemptés de ce warning.

---

## Utilisation

### Vérification sans compilation

```bash
./target/release/ocara mon_fichier.oc --check
```

Affiche toutes les erreurs et warnings sans produire de binaire.

### Voir l'exemple de référence

Le fichier `examples/21_errors.oc` déclenche volontairement 6 erreurs et 5 warnings :

```bash
./target/release/ocara examples/21_errors.oc --check
```

---

## Codes de sortie

| Code | Signification |
|------|--------------|
| `0` | Succès — aucune erreur |
| `1` | Erreur(s) — compilation ou analyse échouée |
