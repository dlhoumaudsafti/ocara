# Ocara — Exemples

Ce dossier contient un script `.oc` par fonctionnalité du langage, des
démonstrations de chaque classe builtin, des applications complètes et la
suite de tests unitaires du langage.

Pour la gestion mémoire des variables (`var`, `const`, `scoped`, `consumed`),
voir [docs/variables.md](../docs/variables.md).

## Exemples principaux

| Fichier | Fonctionnalité |
|---------|---------------|
| [01_variables.oc](01_variables.oc) | `var`, `scoped` (bloc), `consumed` (usage unique), `const` globale, `null` |
| [02_functions.oc](02_functions.oc) | Fonctions, paramètres, retour, récursion |
| [03_builtins.oc](03_builtins.oc) | `IO::writeln` et `IO::read` |
| [04_conditions.oc](04_conditions.oc) | `if` / `elseif` / `else`, opérateurs logiques |
| [05_switch.oc](05_switch.oc) | `switch` sur littéraux, cas `default` |
| [06_match.oc](06_match.oc) | Expression `match` (retourne une valeur) |
| [07_loops.oc](07_loops.oc) | `while`, `for in` (plage et tableau), `for clé has valeur in map` |
| [08_arrays.oc](08_arrays.oc) | Tableaux `array<T>`, `array<mixed>`, multidimensionnels, accès par index |
| [09_maps.oc](09_maps.oc) | Maps `map<K,V>`, littéraux, accès par index, itération |
| [10_classes.oc](10_classes.oc) | Classes, `property`, constructeur `init`, `self`, visibilité |
| [11_interfaces.oc](11_interfaces.oc) | Interfaces, `implements`, polymorphisme |
| [12_inheritance.oc](12_inheritance.oc) | Héritage `extends`, redéfinition de méthode, appel au parent avec `parent::` |
| [13_instantiation.oc](13_instantiation.oc) | Instanciation avec `use` |
| [14_static_access.oc](14_static_access.oc) | Accès statique `Classe::methode()` et `Classe::CONST` |
| [15_operators.oc](15_operators.oc) | Tous les opérateurs et leur précédence |
| [16_types.oc](16_types.oc) | Système de types : primitifs, tableaux, maps, types nommés |
| [17_import.oc](17_import.oc) | Imports et système de modules |
| [18_class_consts.oc](18_class_consts.oc) | Constantes de classe avec visibilité (`public`/`protected`/`private const`) |
| [19_break_continue.oc](19_break_continue.oc) | `break` et `continue` dans les boucles |
| [20_try_fail.oc](20_try_fail.oc) | Gestion des erreurs : `try` / `on` / `raise` |
| [21_errors.oc](21_errors.oc) | Erreurs sémantiques (fichier volontairement invalide, à analyser avec `--check`) |
| [22_union_types.oc](22_union_types.oc) | Types union `T\|null`, retour union, test de nullité |
| [23_static_method.oc](23_static_method.oc) | Appels entre méthodes statiques via `self::` |
| [24_function_types.oc](24_function_types.oc) | Type `Function<Ret(Params)>` : fonctions comme valeurs (variables, paramètres) |
| [25_nameless.oc](25_nameless.oc) | Fonctions anonymes `nameless`, closures, capture de variables et de `self` |
| [26_modules.oc](26_modules.oc) | Modules, imports relatifs et absolus |
| [27_type_narrowing.oc](27_type_narrowing.oc) | Réduction de type avec `is` dans les branches |
| [28_enum.oc](28_enum.oc) | Énumérations : définition, utilisation, `match` |
| [29_async.oc](29_async.oc) | Fonctions `async`, `Resolvable<T>`, `resolve` |
| [30_variadic.oc](30_variadic.oc) | Paramètres variadiques `variadic<T>` |
| [31_default_params.oc](31_default_params.oc) | Paramètres par défaut |
| [32_strict_operators.oc](32_strict_operators.oc) | Comparaisons en toutes lettres (`equal`, `not equal`, `smaller`, `greater`…) |
| [33_increment_decrement.oc](33_increment_decrement.oc) | `i++`, `++i`, `i--`, `--i` |
| [60_interface_wiring.oc](60_interface_wiring.oc) | `wiring` : liaison interface → implémentation à la compilation |
| [61_interface_instance_dispatch.oc](61_interface_instance_dispatch.oc) | Appel de méthode d'instance à travers une interface |
| [62_interface_method_modifiers.oc](62_interface_method_modifiers.oc) | Modificateurs de méthode d'interface (`static`, `async`…) |
| [63_chained_call_on_free_function_result.oc](63_chained_call_on_free_function_result.oc) | Appel chaîné sur le résultat d'une fonction libre |
| [64_async_instance_method_dispatch.oc](64_async_instance_method_dispatch.oc) | Méthode d'instance `async` (`obj.methode()`) |
| [65_async_resolvable_return_type.oc](65_async_resolvable_return_type.oc) | `async` retournant un type quelconque : `Resolvable<T>` |
| [66_chained_call_depth_limit.oc](66_chained_call_depth_limit.oc) | Appels chaînés sur trois niveaux et plus (`a.b().c().d()`) |
| [67_named_arguments.oc](67_named_arguments.oc) | Arguments nommés à l'appel |
| [68_struct.oc](68_struct.oc) | `struct` : agrégat de données, constructeur généré |
| [69_convert_instance_methods.oc](69_convert_instance_methods.oc) | Conversions en méthode d'instance (`s.toInt()`, `n.toStr()`…) |

## Classes builtin (`builtins/`)

| Fichier | Classe |
|---------|--------|
| [builtins/array.oc](builtins/array.oc) | `Array` — manipulation de tableaux |
| [builtins/convert.oc](builtins/convert.oc) | `Convert` — conversions entre types |
| [builtins/date.oc](builtins/date.oc) | `Date` — dates (jour, mois, année) |
| [builtins/datetime.oc](builtins/datetime.oc) | `DateTime` — date et heure, timestamps |
| [builtins/directory.oc](builtins/directory.oc) | `Directory` — création, lecture, suppression de répertoires |
| [builtins/dotenv.oc](builtins/dotenv.oc) | `DotEnv` — chargement de fichiers `.env` |
| [builtins/file.oc](builtins/file.oc) | `File` — lecture et écriture de fichiers |
| [builtins/html.oc](builtins/html.oc) | `HTML` — génération de HTML avec composants et gabarits |
| [builtins/http.oc](builtins/http.oc) | `HTTPRequest` — requêtes HTTP/HTTPS |
| [builtins/httpserver.oc](builtins/httpserver.oc) | `HTTPServer` — serveur HTTP avec routage |
| [builtins/httpserver_session.oc](builtins/httpserver_session.oc) | `HTTPServerSession` — sessions par visiteur et état global partagé |
| [builtins/httpserver_static.oc](builtins/httpserver_static.oc) | `HTTPServer` — fichiers statiques et routage avancé |
| [builtins/io.oc](builtins/io.oc) | `IO` — entrées et sorties standard |
| [builtins/json.oc](builtins/json.oc) | `JSON` — encodage et décodage |
| [builtins/map.oc](builtins/map.oc) | `Map` — manipulation de maps |
| [builtins/math.oc](builtins/math.oc) | `Math` — fonctions et constantes mathématiques |
| [builtins/mutex.oc](builtins/mutex.oc) | `Mutex` — exclusion mutuelle entre threads |
| [builtins/mysql.oc](builtins/mysql.oc) | `MySQL` / `MariaDB` — connexion, requêtes paramétrées, transactions |
| [builtins/regex.oc](builtins/regex.oc) | `Regex` — expressions régulières (POSIX ERE) |
| [builtins/sdl.oc](builtins/sdl.oc) | `SDL` — fenêtre, rendu 2D, événements |
| [builtins/sqlite.oc](builtins/sqlite.oc) | `SQLite` — base embarquée, requêtes paramétrées, transactions |
| [builtins/string.oc](builtins/string.oc) | `String` — manipulation de chaînes |
| [builtins/system.oc](builtins/system.oc) | `System` — OS, PID, variables d'environnement, exécution, arguments |
| [builtins/tauri.oc](builtins/tauri.oc) | `Tauri` — fenêtre desktop native et appels JS → Ocara |
| [builtins/thread.oc](builtins/thread.oc) | `Thread` — création et synchronisation de threads |
| [builtins/time.oc](builtins/time.oc) | `Time` — heures, minutes, secondes |
| [builtins/yaml.oc](builtins/yaml.oc) | `YAML` — encodage et décodage |

Les scripts `builtins/*.sh` lancent les exemples de serveur HTTP.

## Applications complètes (`advanced/`)

> Ces applications structurent leur point d'entrée avec les blocs de cycle
> de vie `init`/`main`/`error`/`success`/`exit` (variables `ERROR`/`SUCCESS`,
> mot-clé `result`) plutôt qu'avec `function main(): int`. Voir
> [docs/EBNF.md §5 « Blocs runtime »](../docs/EBNF.md#5-blocs-runtime).

| Dossier | Contenu |
|---------|---------|
| [advanced/httpserver/](advanced/httpserver/) | Site web : routage, contrôleurs, composants HTML, fichiers statiques |
| [advanced/tauri_httpserver/](advanced/tauri_httpserver/) | Le même site dans une application desktop Tauri (serveur sur un thread dédié) |
| [advanced/mini_project/](advanced/mini_project/) | Gestion de flotte de voitures : SQLite, services, gabarits, Tauri |
| [advanced/mini_project_hexa/](advanced/mini_project_hexa/) | Le même projet en architecture hexagonale (contextes, contrats `wiring`, `async`, sessions) — `probe_main.oc` démarre le serveur seul, sans fenêtre |
| [advanced/game_sdl/](advanced/game_sdl/) | Jeu de plateforme 2D en défilement horizontal avec `SDL` (voir son README) |

## Génériques, imports et modules

| Dossier / fichier | Contenu |
|---------|---------|
| [generics/](generics/) | Classe générique `List<T>` et son usage |
| [from/import_from.oc](from/import_from.oc) | Imports sélectifs `from … import` de classes, interfaces et modules |
| [mods/](mods/) | Modules réutilisables : `Logger`, `Math`, `User` |

## Projet multi-fichiers (`project/`)

| Fichier | Contenu |
|---------|---------|
| [project/main.oc](project/main.oc) | Point d'entrée, classes `Score` et `Student` |
| [project/classes/](project/classes/) | Modèles, services et utilitaires |
| [project/tests/](project/tests/) | Tests unitaires du projet (`mainTest`, `ModelsTest`, `ServicesTest`, `UtilsTest`, `AliasClassInheritanceTest`) |

## Tests unitaires du langage (`tests/`)

Exécutés par `ocaraunit` (classe `UnitTest`) dans `make regression`.

| Fichiers | Ce qui est testé |
|---------|-----------------|
| `01`–`32` | Une suite par exemple principal du même numéro |
| [33_exception_hierarchyTest](tests/33_exception_hierarchyTest.oc) | `on e is X` attrape les sous-classes de `X` |
| [34_string_nul_safetyTest](tests/34_string_nul_safetyTest.oc) | Chaînes contenant un caractère NUL |
| [35_var_auto_freeTest](tests/35_var_auto_freeTest.oc) | Libération des valeurs portées par des `var` |
| [36_mutex_withlockTest](tests/36_mutex_withlockTest.oc) | `Mutex::withLock` : déverrouillage garanti, même sur exception |
| [37_mixed_large_intTest](tests/37_mixed_large_intTest.oc), [38](tests/38_mixed_call_arg_boxingTest.oc) | Grands entiers dans un `mixed`, boxing des arguments |
| [39_polymorphismTest](tests/39_polymorphismTest.oc) | Dispatch dynamique via une classe parente ou une interface |
| [40](tests/40_json_yaml_concrete_containersTest.oc), [74](tests/74_encode_raw_leaf_shapeTest.oc) | `JSON`/`YAML` sur des conteneurs typés |
| [41_nested_container_ownershipTest](tests/41_nested_container_ownershipTest.oc) | Conteneurs imbriqués à éléments scalaires |
| [42_emit_generatorTest](tests/42_emit_generatorTest.oc) | Générateurs `emit` / `message<T>` |
| [43](tests/43_array_map_get_concrete_typeTest.oc), [50](tests/50_chained_index_on_mapTest.oc), [58](tests/58_mixed_container_indexed_assignment_boxingTest.oc), [70](tests/70_typed_container_literalsTest.oc) | Accès et littéraux de conteneurs typés, indexation chaînée |
| [44](tests/44_string_json_sugar_parityTest.oc), [69](tests/69_convert_instance_methodsTest.oc) | Parité forme statique / méthode d'instance |
| [45](tests/45_increment_decrementTest.oc), [76](tests/76_compound_assignmentTest.oc) | `++`/`--` et affectations composées |
| [46](tests/46_nested_closure_recaptureTest.oc)–[48](tests/48_closure_promotion_in_loopTest.oc) | Closures imbriquées, créées dans un bloc ou une boucle |
| [49](tests/49_sqlite_with_open_raiseTest.oc), [55](tests/55_sqlite_real_column_boxingTest.oc), [57](tests/57_sqlite_integer_column_boxingTest.oc) | SQLite : `withOpen` et exceptions, types des colonnes |
| [51_import_alias_and_self_constTest](tests/51_import_alias_and_self_constTest.oc) | Alias d'import de builtin, `self::CONST` |
| [52_class_resource_property_destructorTest](tests/52_class_resource_property_destructorTest.oc) | Champ ressource fermé avec son objet |
| [53](tests/53_use_chained_methodTest.oc), [54](tests/54_httprequest_chained_methodTest.oc), [63](tests/63_chained_call_on_free_function_resultTest.oc), [66](tests/66_chained_call_depth_limitTest.oc) | Appels chaînés |
| [56_union_class_null_field_accessTest](tests/56_union_class_null_field_accessTest.oc) | Champs d'une variable `Classe\|null` |
| [59](tests/59_httpserver_requestTest.oc), [73](tests/73_httpserver_sessionTest.oc) | `HTTPServerRequest`, sessions |
| [60](tests/60_interface_wiringTest.oc)–[62](tests/62_interface_method_modifiersTest.oc) | `wiring`, dispatch d'interface, modificateurs |
| [64](tests/64_async_instance_method_dispatchTest.oc), [65](tests/65_async_resolvable_return_typeTest.oc) | `async` et `Resolvable<T>` |
| [67_named_argumentsTest](tests/67_named_argumentsTest.oc) | Arguments nommés |
| [68_structTest](tests/68_structTest.oc) | `struct` |
| [71_variadic_forwarding_and_pushTest](tests/71_variadic_forwarding_and_pushTest.oc) | Transmission de variadiques |
| [72_property_initializerTest](tests/72_property_initializerTest.oc) | Valeur par défaut d'une `property` |
| [75_map_loop_value_typeTest](tests/75_map_loop_value_typeTest.oc) | Type de la valeur dans `for k has v in m` |
| [77](tests/77_builtin_exception_constructorTest.oc), [78](tests/78_raised_string_interpolationTest.oc) | Constructeur des exceptions builtin, chaîne levée interpolée |
| [79](tests/79_field_loop_match_and_kept_elementsTest.oc)–[83](tests/83_object_ownership_aliasesTest.oc) | Conteneurs d'objets : partage, transferts, alias, éléments conservés |
| [84_refcountTest](tests/84_refcountTest.oc) | Comptage de références : partage, `scoped`/`consumed`, closures, exceptions, `async` |

## Compiler et exécuter un exemple

```bash
# Depuis la racine du projet
make build

./target/release/ocara examples/01_variables.oc -o out && ./out
./target/release/ocara examples/07_loops.oc -o out && ./out

# Vérification sémantique uniquement
./target/release/ocara examples/10_classes.oc --check

# Tokens et AST
./target/release/ocara examples/06_match.oc --dump

# Un fichier de tests
./target/release/ocaraunit examples/tests/84_refcountTest.oc

# Régression complète (exemples + tests)
make regression

# Un seul exemple
make regression 07_loops
make regression builtins/http
```

Pour mesurer la mémoire ou le temps d'un programme, voir
[docs/benchmarking.md](../docs/benchmarking.md).
