# Ocara v1.0 — Exemples

Ce dossier contient un script `.oc` par fonctionnalité du langage.

## Exemples principaux

| Fichier | Fonctionnalité |
|---------|---------------|
| [01_variables.oc](01_variables.oc) | `var` (mutable, portée fonction), `scoped` (mutable, portée bloc), `const` (globale) |
| [02_functions.oc](02_functions.oc) | Fonctions, paramètres, retour, récursion |
| [03_builtins.oc](03_builtins.oc) | `IO::writeln` et `IO::read` |
| [04_conditions.oc](04_conditions.oc) | `if` / `elseif` / `else`, opérateurs logiques |
| [05_switch.oc](05_switch.oc) | `switch` sur littéraux, cas `default` |
| [06_match.oc](06_match.oc) | Expression `match` (retourne une valeur) |
| [07_loops.oc](07_loops.oc) | `while`, `for in` (range et tableau), `for key => val` |
| [08_arrays.oc](08_arrays.oc) | Tableaux `T[]`, multidimensionnels, `any[]`, accès index |
| [09_maps.oc](09_maps.oc) | Maps `map<K,V>`, littéraux, accès index, itération |
| [10_classes.oc](10_classes.oc) | Classes, `property`, constructeur `init`, `self`, visibilité |
| [11_interfaces.oc](11_interfaces.oc) | Interfaces, `implements`, polymorphisme |
| [12_inheritance.oc](12_inheritance.oc) | Héritage `extends`, surcharge de méthode |
| [13_instantiation.oc](13_instantiation.oc) | Instanciation avec `use` |
| [14_static_access.oc](14_static_access.oc) | Accès statique `Class::method()` + `Class::CONST` |
| [15_operators.oc](15_operators.oc) | Tous les opérateurs et leur précédence |
| [16_types.oc](16_types.oc) | Système de types : primitifs, tableaux, maps, nommés |
| [17_modules.oc](17_modules.oc) | Imports et système de modules |
| [18_class_consts.oc](18_class_consts.oc) | Constantes de classe avec visibilité (`public/protected/private const`) |
| [19_break_continue.oc](19_break_continue.oc) | `break` et `continue` dans les boucles |
| [20_try_fail.oc](20_try_fail.oc) | Gestion des erreurs : `try` / `on` / `raise` |

## Classes builtins (`builtins/`)

| Fichier | Classe |
|---------|--------|
| [builtins/array.oc](builtins/array.oc) | `Array` — manipulation de tableaux |
| [builtins/convert.oc](builtins/convert.oc) | `Convert` — conversions entre types |
| [builtins/http.oc](builtins/http.oc) | `HTTPRequest` — requêtes HTTP/HTTPS |
| [builtins/io.oc](builtins/io.oc) | `IO` — entrées / sorties standard |
| [builtins/map.oc](builtins/map.oc) | `Map` — manipulation de maps |
| [builtins/math.oc](builtins/math.oc) | `Math` — fonctions et constantes mathématiques |
| [builtins/regex.oc](builtins/regex.oc) | `Regex` — expressions régulières (POSIX ERE) |
| [builtins/string.oc](builtins/string.oc) | `String` — manipulation de chaînes |
| [builtins/system.oc](builtins/system.oc) | `System` — OS, PID, env, exec, args… |

## Projet multi-fichiers (`project/`)

Exemple complet illustrant imports, classes, modules et toutes les fonctionnalités du langage en situation réelle.

## Compiler et exécuter un exemple

```bash
# Depuis la racine du projet
make build

./target/release/ocara examples/01_variables.oc -o out && ./out
./target/release/ocara examples/07_loops.oc -o out && ./out

# Vérification sémantique uniquement
./target/release/ocara examples/10_classes.oc --check

# Mode diagnostic (tokens + AST + HIR)
./target/release/ocara examples/06_match.oc --dump

# Régression complète
make regression

# Un seul exemple
make regression 07_loops
make regression builtins/http
```

