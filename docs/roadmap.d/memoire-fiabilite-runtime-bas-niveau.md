# Fragilités bas niveau du runtime mémoire

Quatre points latents identifiés initialement — tous corrigés.

## ✅ Tag d'exception confondu avec `TAG_MAP` — corrigé

Le tag `0x03` utilisé pour les exceptions (`alloc_exception`, `runtime/src/exception.rs`) était **littéralement identique** à `TAG_MAP` (`runtime/src/typecheck.rs`) — pas juste une collision théorique : confirmé par reproduction, une `ArrayException` capturée par un `on e { }` répondait `true` à `e is map<string, mixed>`.

```ocara
try {
    var x:int = arr.pop()   // arr vide → ArrayException
} on e {
    if e is map<string, mixed> {
        IO::writeln("MISCLASSIFIED as map!")   // s'affichait avant ce correctif
    }
}
```

**Corrigé** : nouveau tag dédié `TAG_EXCEPTION = 7` (`runtime/src/typecheck.rs`), utilisé par `alloc_exception` à la place de `0x03`. Vérifié : le cas ci-dessus répond maintenant correctement `false` ; `get_value_type` (utilisé par `JSON::encode`/comparaisons strictes) retombe correctement sur "primitif" pour ce nouveau tag, comme pour tout tag non reconnu — aucune exception n'étant aujourd'hui `scoped`/`consumed`, `__value_free`/`__value_clone` ne sont pas concernés.

## ✅ Portabilité `Mutex` — corrigée

`PthreadMutex` était un tableau d'octets de taille hardcodée par plateforme (`[u8;40]` Linux, `[u8;64]` macOS/fallback), jamais vérifiée contre la vraie taille de `pthread_mutex_t` de l'ABI cible — une libc dont la structure serait plus grande (ex. une musl ou une variante Linux différente de celle supposée) aurait laissé `pthread_mutex_init` écrire hors des bornes de l'allocation.

**Corrigé** : remplacement par `libc::pthread_mutex_t` (nouvelle dépendance `libc` dans `runtime/Cargo.toml`), dont la taille/l'alignement sont garantis corrects pour la cible de compilation réelle. Les 5 fonctions `pthread_mutex_*` appelées à la main sont aussi remplacées par celles de `libc` directement (au lieu de redéclarations manuelles risquant de diverger de la signature réelle). Vérifié : `make regression` (exemple `mutex.oc`, double-`.destroy()` E25) sans régression.

## ✅ Corrigé — `free_str` ne recalcule plus une taille potentiellement fausse

**Des deux fonctions initialement citées ici, une seule portait un risque réel** : `free_str` retrouvait la longueur d'une string en cherchant son premier octet NUL depuis le pointeur — sous-estimation garantie si la string contient un NUL **interne**, ce qui est un cas atteignable (`\0` est un échappement de chaîne Ocara valide, `"a\0b"` compile et alloue normalement). Une longueur sous-estimée passe un `Layout` trop court à `dealloc` : UB (l'API Rust `alloc`/`dealloc` exige que le `Layout` de libération corresponde exactement à celui de l'allocation), potentiellement une corruption du tas.

`__object_free` (le second cas cité), en réexamen, ne portait en réalité **aucun risque de divergence** : `n_fields` provient d'une seule et même source (`module.class_layouts[Classe].len()`, immuable après compilation), relue identiquement au moment de l'allocation (`Inst::Alloc`, `src/codegen/emit.d/instructions.d/memory.rs`) et au moment de la libération (`src/lower/builder.d/class_ownership.rs`) — les deux lectures ne peuvent pas diverger au sein d'un même programme compilé. Laissé tel quel : pas de changement nécessaire, le risque était mal caractérisé au moment où ce point a été noté.

**Corrigé** (`alloc_str`/`free_str`, `runtime/src/lib.rs`) — sans le changement de format de header global initialement envisagé (qui aurait touché `Inst::Alloc` et toutes les autres allocations heap, cf. ancienne version de cette section) : uniquement le layout d'une string possédée gagne une case de 8 octets, AVANT le tag existant (qui reste à l'offset habituel `val - 8`, donc invisible de `read_tag`/`ptr_to_str`/tout le reste du runtime) :

```
avant : [tag:8][données...][NUL]
après : [len:8][tag:8][données...][NUL]
```

`free_str` lit maintenant `len` directement au lieu de le recalculer. Vérifié : régression complète sans changement (le format ne change le comportement OBSERVABLE d'aucun appelant, seul `free_str` lit `len`) ; un test dédié (`examples/tests/34_string_nul_safetyTest.oc`) alloue/libère en boucle des `scoped string` contenant un NUL interne, entrelacées avec d'autres allocations de taille voisine, sans crash. **Non vérifié empiriquement** : que l'ancien code plantait réellement sur ce système (glibc `free()` ne vérifie pas nécessairement la taille annoncée) — `valgrind` (qui l'aurait détecté à coup sûr) n'est pas disponible dans cet environnement ; la correction reste justifiée par le contrat documenté de `std::alloc::{alloc, dealloc}`, indépendamment de la démonstration empirique.

**Volontairement non traité, hors périmètre** : `ptr_to_str` (et donc l'affichage, la comparaison, `String::*`, `JSON::encode`, ...) reste basé sur `CStr::from_ptr`, qui tronque toujours au premier NUL — une string à NUL interne reste donc **affichée/comparée tronquée** partout ailleurs dans le langage. Ce correctif ferme uniquement le risque de corruption mémoire à la libération ; rendre le contenu d'une telle string réellement correct de bout en bout demanderait une représentation de string à longueur explicite (pas seulement NUL-terminée), un changement bien plus large que ce point.

## ✅ Détection de type par heuristique sur la valeur d'un entier — corrigé

`read_tag`/`get_value_type` traitaient toute valeur `>= 65536` avec les bits bas à `00` (ou 8-alignée pour `get_value_type`) comme un pointeur heap valide et la déréférençaient sans vérification — un **SEGFAULT reproductible** sur du code parfaitement ordinaire :

```ocara
var n:mixed = 1000000
if n is string { ... }   // SEGFAULT : déréférence l'adresse (1000000 - 8)
```

Les deux options envisagées initialement (revoir toute la représentation des entiers/pointeurs, ou vérifier qu'une page est réellement mappée via `mincore()` à chaque déréférencement) ont été écartées comme trop coûteuses/invasives. **Solution retenue**, plus chirurgicale : étendre le mécanisme de boxing à pointeur tagué déjà en place pour `float`/`bool` (2 bits bas encodant le type — `01`=float, `10`=bool — la combinaison `11` était inutilisée) avec un troisième cas, `11` = **int boxé** :

- `box_int_if_needed(n)` (`runtime/src/lib.rs`) : ne boxe QUE si `n >= 65536` (le seuil `PTR_THRESHOLD` déjà utilisé partout ailleurs) — un entier négatif ou petit, jamais ambigu avec un pointeur, reste brut, sans coût d'allocation. `__box_int_for_mixed`/`__unbox_int` sont les points d'entrée C exportés.
- Tous les consommateurs existants de float/bool boxés étendus symétriquement pour reconnaître aussi un int boxé : `val_to_string`/`__val_to_str` (affichage), `unbox_numeric_i64`/`unbox_numeric_f64` (arithmétique `mixed`, `__dyn_add`/`sub`/`mul`/`div` — dont le résultat entier est lui-même reboxé si nécessaire avant de retourner, puisque rien en aval ne le referait), `__is_int` (narrowing `is int`), `value_to_json`/`json_to_value` (`JSON::encode`/`decode`), `value_to_yaml`/`yaml_to_value` (`YAML::encode`/`decode`).
- **Comparaisons strictes** (`equal`/`not equal`/`smaller`/`greater`/`smaller or equal`/`greater or equal`, `__cmp_*_strict`) : corrigées pour déballer un opérande boxé (int OU float — les deux souffraient du même bug, la comparaison se faisait auparavant sur l'adresse boxée elle-même) via un nouvel helper partagé `cmp_primitive`, plutôt qu'une comparaison brute de bits — corrige au passage un bug préexistant sur les floats boxés, pas seulement le nouveau cas int.
- Tous les points de lowering qui boxaient déjà F64/Bool à l'entrée d'un `mixed` (affectation `var`/`const`/assignation, argument de fonction libre/constructeur, empaquetage `variadic<mixed>`, élément de littéral `array<mixed>`/`map<K,mixed>`, opérande d'une comparaison stricte) étendus avec le même traitement pour I64.

**Plusieurs bugs latents, plus larges que prévu, découverts et corrigés en cours de route** (tous des cas où le lowering supposait à tort qu'une valeur était déjà dans sa représentation "mixed" correcte, sans jamais l'avoir été) :

1. **`expr_ir_type` retournait `I64` par défaut** pour plusieurs formes d'expression jamais couvertes explicitement (`Expr::Array`, `Expr::Map`, `Expr::New`, `self`, `parent`) — strictement inoffensif tant que rien n'agissait différemment selon I64/Ptr, mais désormais dangereux : un `var a:array<int> = [1,2,3]` voyait son PROPRE POINTEUR boxé comme si c'était un entier (`Array::len(a)` retournait n'importe quoi). Corrigé en ajoutant les arms explicites manquants (toujours `Ptr`, ces expressions sont toujours des pointeurs tas).
2. **`expr_ir_type` retombait sur `I64` pour tout appel statique (`Class::method()`) absent de la table `fn_ret_types`** — `SQLite::open`/`MySQL::connect`/`MariaDB::connect` (jamais ajoutés à cette table, un oubli préexistant) en faisaient partie : le pointeur de connexion lui-même se faisait boxer, corrompant `self` et bloquant `db.execute()` dans une boucle infinie (confirmé par reproduction, CPU à 100%). Corrigé à deux niveaux : le filet de sécurité générique retombe désormais sur `Ptr` (jamais sur `I64` — un pointeur objet mal classé en `Ptr` reste inoffensif, l'inverse corrompt) ; et les entrées manquantes pour `SQLite`/`MySQL`/`MariaDB` ont été ajoutées explicitement à `fn_ret_types` (`src/lower/builder.d/program.rs`), pratique déjà établie pour le reste de cette table.
3. **La libération/le clonage automatiques d'un `var`/`scoped`/`consumed array<T>`/`map<K,T>` à élément PRIMITIF CONCRET (`int`/`float`/`bool`, jamais `mixed`) inspectaient quand même chaque élément via `__value_free`/`__value_clone`** (tag runtime), en supposant à tort qu'un élément pouvait être un pointeur tas imbriqué — un `float`/`int` brut peut avoir n'importe quel bit pattern, y compris un qui ressemble à un pointeur heap valide. SEGFAULT confirmé par reproduction sur du code n'utilisant `mixed` nulle part :
   ```ocara
   var floats:array<float> = [1.5, 2.5, 3.5]   // jamais échappé
   // ... plantait à la libération automatique de fin de bloc
   ```
   Corrigé par de nouvelles variantes "shallow" (`__array_free_shallow`/`__map_free_shallow`/`__array_clone_shallow`/`__map_clone_shallow`, `runtime/src/lib.rs`) qui ne parcourent pas les éléments — choisies par le lowering (`drop_func_for`/`clone_func_for`, `src/lower/stmt.d/ownership.rs`) dès que l'élément est un primitif concret. **Limite assumée et non traitée** : un conteneur imbriqué à plusieurs niveaux (`array<array<int>>`) reste sur le chemin récursif générique au niveau externe — seul le niveau immédiat de chaque `var`/`scoped`/`consumed` est corrigé ; un tableau interne `array<int>` atteint via cette récursion resterait exposé au même risque. Non rencontré dans aucun exemple existant, mais pas prouvé impossible.

**Un quatrième bug, plus large encore, découvert PENDANT la vérification finale** (pas en creusant le code cette fois, mais parce que `make regression` s'est mis à échouer/bloquer sur des exemples sans aucun rapport avec `mixed`) : `expr_ir_type` (la même fonction corrigée au point 1/2 ci-dessus) retombe, pour un appel `Class::méthode()`/`objet.méthode()` absent de la table `fn_ret_types`, sur un filet de sécurité — auparavant `I64`, dorénavant `Ptr` (voir point 2). Cette table s'est révélée **très incomplète** pour plusieurs classes builtin entières, jamais remarqué avant que le filet de sécurité n'ait un effet réel :
- `SQLite::open`/`MySQL::connect`/`MariaDB::connect` : absents → `db.execute(...)` bloquait dans une boucle infinie (self-pointer corrompu, CPU à 100%, confirmé et corrigé — voir point 2).
- `Math::sqrt`, `Convert::strToFloat`/`intToFloat`/`boolToFloat`, `IO::readFloat` : absents, et retournent réellement un `F64` — un vrai crash différent (mauvaise classe de registre, valeur numériquement incohérente, ex. `Math::sqrt(16.0)` affichait `4616189618054758000` au lieu de `4`), pas une histoire de boxing.
- `Convert::strToInt`/`strToBool`/`floatToInt`/`boolToInt`/..., `IO::readInt`/`readBool`, `System::pid`/`passthrough`/`execCode` : absents, retournent réellement `I64`/`Bool` — inoffensif tant que la valeur reste petite, mais `System::pid()` (souvent une valeur `>= 65536` sur ce système) reproduisait le SEGFAULT documenté au point 1 dès qu'elle traversait un template `${...}`.
- `Expr::Index` (`attrs["title"]` sur un paramètre `map<string,mixed>` d'une closure `nameless`, jamais enregistré dans `elem_types` contrairement à un paramètre de fonction top-level) : même filet de sécurité, même correctif (`Ptr` au lieu de `I64`) — `examples/advanced/httpserver` affichait l'adresse d'un pointeur string à la place du titre de page.

**Corrigé** : entrées explicites ajoutées à `fn_ret_types` pour `SQLite`/`MySQL`/`MariaDB` (constructeurs + `query`/`queryOne`), `Convert::*` (19 méthodes, chacune par son vrai type), `Math::*` (10 méthodes), `IO::read*` (7 méthodes), `System::pid`/`passthrough`/`execCode`/`args` — remplaçant au passage plusieurs raccourcis par préfixe de chaîne (`fname.starts_with("Convert_")`, `"IO_read"`) qui classaient à tort CERTAINES méthodes du bon groupe comme `Ptr` alors qu'elles retournent un type concret. Le filet de sécurité générique de `expr_ir_type` (`Expr::StaticCall`, `Expr::Call`, `Expr::Index`) retombe désormais sur `Ptr` plutôt que `I64` dans tous les cas encore non couverts.

**Vérifié** : la reproduction documentée à l'origine (`var n:mixed = 1000000; if n is string {...}`) ne plante plus, `n is int` répond correctement `true`. `examples/tests/37_mixed_large_intTest.oc` (25 assertions : narrowing `is`, entier non aligné sur 4 mais quand même boxé, petit entier jamais boxé, entier négatif de grande magnitude jamais ambigu, réaffectation mixed↔string↔int, arithmétique dynamique avec reboxing du résultat, comparaisons strictes, round-trip JSON, régression directe sur `array<float>`/`array<int>` concrets). `examples/16_types.oc` (exemple historique qui plantait déjà avant ce chantier, via `display_all([42, "hello", true])` puis la libération de fin de bloc de ses propres tableaux), `examples/builtins/{convert,math,io,system,json}.oc` et `examples/advanced/httpserver` (tous cassés à un moment de cette passe, par le bug `fn_ret_types` ci-dessus — pas par le boxing int lui-même) s'exécutent maintenant intégralement. `make regression` final : **447 PASS / 0 FAIL / 0 ERREUR** côté `ocaraunit` (était 422 avant ce chantier — +8 `Mutex::withLock`, +25 ce point), et tous les exemples `examples/*.oc`/`examples/builtins/*.oc`/`examples/advanced/*` verts, aucun SEGFAULT résiduel.

## ✅ Argument d'appel de méthode (instance/statique) jamais boxé vers un paramètre `mixed` — corrigé

Découvert en creusant le chantier ci-dessus, distinct du reste : un appel de fonction libre ou un constructeur boxait déjà correctement un argument F64/Bool/I64 vers un paramètre `mixed`, mais **un appel de méthode d'instance (`obj.method(...)`) ou statique (`Class::method(...)`) ne boxait rien du tout**, qu'il s'agisse d'une méthode utilisateur ou builtin — confirmé par reproduction :

```ocara
class Box {
    public method show(v:mixed): void {
        IO::writeln(v is float)   // affichait "false"
        IO::writeln(v)            // puis SEGFAULT
    }
}
var b:Box = use Box()
b.show(3.5)
```

**Corrigé** : la décision de boxing (déjà écrite pour `box_for_any`/`box_for_dyn_arith`) est factorisée dans un nouvel helper partagé, `box_arg_for_mixed_param` (`src/lower/expr.d/helpers.rs`), appliqué aux quatre sites d'appel (fonction libre, constructeur — refactorés sans changement de comportement — méthode d'instance et méthode statique — nouveaux). Le type du paramètre visé est résolu via `param_type_for_call_arg`, qui cherche dans l'ordre : `LowerBuilder::fn_param_types` (fonctions libres + méthodes statiques utilisateur, déjà existant), une nouvelle table `IrModule::method_param_types` (méthodes D'INSTANCE utilisateur — absentes de toute table jusqu'ici, `program.rs` ne collectait leurs valeurs par défaut que sans leurs types), puis `builtin_method_param_types()` (toute méthode builtin, statique ou d'instance, construite une fois depuis `crate::builtins::all_builtins()`). Cette dernière table est délibérément SÉPARÉE de `fn_param_types` : celle-ci alimente aussi la génération des wrappers `__fn_wrap_*` pour les fonctions référençables comme valeur — y ajouter les ~centaines de méthodes builtin aurait généré autant de wrappers jamais utilisés (et un wrapper généré pour une méthode d'instance, qui ignorerait `self`, aurait de toute façon été cassé).

**Régression sérieuse détectée et corrigée en cours de route** : plusieurs méthodes `UnitTest::assert*` sont écrites en Rust et lisaient directement le bit brut de leur paramètre `mixed` plutôt que par les accesseurs "boxing-aware" (`cmp_primitive`/`unbox_numeric_i64`, déjà introduits plus haut) — un bool boxé étant un pointeur heap (donc TOUJOURS non nul, que la valeur logique soit `true` ou `false`), le boxing tout neuf des arguments de méthode a cassé `assertTrue`/`assertFalse` UNIVERSELLEMENT sur toute expression bool, dans TOUTE la suite de tests (`make regression` a immédiatement viré au rouge). Corrigées :
- `assertTrue`/`assertFalse` : déballent maintenant via `unbox_numeric_i64` avant le test `!= 0`/`== 0`.
- `assertGreater`/`assertLess`/`assertGreaterOrEquals`/`assertLessOrEquals` : comparaient aussi en brut (`a > b` sur les bits) — déballent maintenant via `cmp_primitive`.
- `assertEmpty`/`assertNotEmpty` : appelaient `ptr_to_str` sur TOUTE valeur `>= 0x10000`, y compris une valeur boxée (dont l'adresse, décalée du tag, ne pointe pas vers une vraie string — SEGFAULT potentiel, même famille de bug que le point 1 ci-dessus) — un boxé est maintenant explicitement traité comme "non vide" avant cette heuristique.
- `assertEquals`/`assertNotEquals` : comparaient par égalité BRUTE de pointeur (bug préexistant, documenté plus tôt dans ce chantier, qui ne "marchait" pour deux strings que par coïncidence de pointeur, et confondait deux `mixed` boxés représentant le même nombre) — corrigées en délégant à `__cmp_eq_strict` (déjà correcte pour tous les cas). `ut_val_to_display` (messages d'échec) avait aussi sa PROPRE réimplémentation, cassée, du déballage float/bool boxé (traitait la valeur comme encodant directement les bits du float, alors que le boxing alloue une vraie cellule tas) — remplacée par un appel à `val_to_string`, déjà correcte.

**Non exploré plus avant** : `assertContains` appelle aussi `ptr_to_str` sans garde sur ses deux arguments `mixed` — même risque théorique que `assertEmpty` si jamais appelée avec une valeur numérique boxée, mais son usage réel (comparaison de sous-chaînes) rend ce cas peu probable ; laissé tel quel. Plus généralement, d'autres builtins `mixed` non audités ici pourraient partager la même hypothèse implicite de représentation brute — si l'un d'eux régresse après avoir touché `builtin_method_param_types`, la même correction (le rendre boxing-aware, pas retirer le boxing) est la bonne réponse, voir le commentaire de cette table dans `src/lower/expr.d/helpers.rs`.

**Vérifié** : la reproduction ci-dessus ne plante plus (`b.show(3.5)` affiche correctement `true` puis `3.5`), y compris pour une méthode STATIQUE utilisateur, une méthode builtin statique (`Array::push`), et un constructeur. Nouveau test dédié (`examples/tests/38_mixed_call_arg_boxingTest.oc`, 11 assertions : méthode d'instance/statique float/bool/grand entier, constructeur, méthode builtin statique). `make regression` final : **481 PASS / 0 FAIL / 0 ERREUR**, aucun SEGFAULT résiduel, aucune régression sur la suite de tests existante malgré la casse temporaire d'`assertTrue`/`assertFalse` en cours de route.

## Fichiers clés

`runtime/src/typecheck.rs` (`TAG_EXCEPTION`, `read_tag`, `__is_int`), `runtime/src/exception.rs` (`alloc_exception`), `runtime/src/mutex.rs` (`libc::pthread_mutex_t`), `runtime/Cargo.toml` (dépendance `libc`), `runtime/src/lib.rs` (`box_int_if_needed`/`__box_int_for_mixed`/`__unbox_int`, `cmp_primitive`, `__array_free_shallow`/`__map_free_shallow`/`__array_clone_shallow`/`__map_clone_shallow`, `value_to_json`/`json_to_value`, `UnitTest_assertTrue`/`assertFalse`/`assertEquals`/`assertNotEquals`/`assertGreater`/`assertLess`/`assertGreaterOrEquals`/`assertLessOrEquals`/`assertEmpty`/`assertNotEmpty`, `ut_val_to_display`), `runtime/src/yaml.rs` (`value_to_yaml`/`yaml_to_value`), `src/lower/stmt.d/statements.d/helpers.rs` (`box_for_any`), `src/lower/expr.d/lower.rs` (`box_for_dyn_arith`, méthode d'instance, appel statique, constructeur), `src/lower/expr.d/helpers.rs` (`box_arg_for_mixed_param`, `param_type_for_call_arg`, `builtin_method_param_types`), `src/lower/expr.d/typeinfer.rs` (`expr_ir_type`), `src/lower/stmt.d/ownership.rs` (`drop_func_for`/`clone_func_for`), `src/lower/builder.d/program.rs` (`fn_ret_types`, méthodes d'instance utilisateur), `src/ir/module.rs` (`method_param_types`), `src/codegen/desc.d/lowlevel.rs`.
