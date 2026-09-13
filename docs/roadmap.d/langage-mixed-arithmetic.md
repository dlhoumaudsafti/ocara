# Arithmétique incorrecte entre `int`/`float` et une valeur `mixed`

## ✅ Corrigé

## Constat (avant correctif)

Découvert en écrivant la documentation de `Map::forEach` (voir [builtins-map-foreach](builtins-map-foreach.md)). Une addition (`+`) entre un `int` et une variable `mixed` qui contient réellement un entier produisait un résultat numériquement faux — silencieusement, sans erreur ni avertissement au-delà du warning habituel "type 'mixed' disables type checking".

## Reproduction (avant correctif)

```ocara
var x:mixed = 95
var y:int = 10
var z:int = y + x
IO::writeln(z)   // affichait 1062449896 au lieu de 105
```

**Cause réelle** (différente de l'hypothèse initiale) : `+` traitait TOUT opérande `Ptr` (le type IR auquel `mixed` est toujours réduit, voir `IrType::from_ast`) comme une concaténation string inconditionnelle — y compris un `mixed` contenant en réalité un entier stocké brut (jamais boxé, sous le seuil `PTR_THRESHOLD`, voir `box_for_any`). `__str_concat(10, 95)` stringifiait chaque opérande individuellement (correctement : `"10"` et `"95"`) puis les concaténait en `"1095"` — un vrai pointeur heap, stocké tel quel dans `z:int`. `-`/`*`/`/` n'avaient PAS ce problème pour un `mixed` contenant un entier (aucune interception spéciale, l'entier brut est déjà numériquement correct) — mais un `mixed` contenant un **float ou un bool boxé** (`__box_float`/`__box_bool`, tag dans les 2 bits bas) produisait, lui, un résultat tout aussi faux pour `-`/`*`/`/`/`%` **et** pour une simple affectation directe (`var f:float = mixedValue`, sans arithmétique) : le bit pattern du pointeur boxé était utilisé/stocké tel quel.

## Correctif

Nouveau dispatch runtime dynamique, sur le même principe que les comparaisons strictes (`__cmp_eq_strict`...) déjà existantes, mais pour l'arithmétique — `runtime/src/lib.rs` :
- **`__dyn_add(a, b)`** — décide RÉELLEMENT au runtime : concaténation si un côté est un vrai objet tas (string/array/map/objet/fonction — via `get_value_type`, comportement historique préservé), addition numérique sinon (flottante si un côté est un float boxé, entière sinon).
- **`__dyn_sub`/`__dyn_mul`/`__dyn_div`** — même principe sans la branche concaténation (`-`/`*`/`/` n'ont jamais de sens sur un vrai objet tas) : entier ou flottant décidé dynamiquement (`is_float_box`), plutôt que de figer ça au type statique de l'AUTRE opérande — un `int` connu combiné à un `mixed` contenant en réalité un `float` aurait sinon tronqué silencieusement ce float en entier (limitation intermédiaire découverte et corrigée en cours de chantier, voir la reproduction `d / e` ci-dessous).
- **`%`** reste toujours entier (`Inst::Mod` n'a de toute façon aucun support flottant dans ce compilateur, `srem` inconditionnel) — un opérande `mixed` y est simplement déballé en entier (`__mixed_to_int`).
- **`__mixed_to_int`/`__mixed_to_float`** — déballage générique d'un `Ptr` potentiellement `mixed` (entier brut déjà correct, float/bool boxé reconverti) vers une cible numérique concrète. Réutilisé par `box_for_any` (`src/lower/stmt.d/statements.d/helpers.rs`) pour combler le sens manquant : jusqu'ici cette fonction ne savait que BOXER (`int`/`float`/`bool` connu → `mixed`), jamais DÉBALLER (`mixed` → `int`/`float`/`bool` connu) — confirmé par une reproduction indépendante de toute arithmétique (`var m:mixed = 3.5; var f:float = m` affichait un nombre dénormalisé proche de zéro au lieu de `3.5`).

Câblage côté compilateur (`src/lower/expr.d/lower.rs`, `src/lower/expr.d/typeinfer.rs`) :
- `+`/`-`/`*`/`/` avec au moins un opérande `Ptr` appellent désormais `__dyn_add`/`__dyn_sub`/`__dyn_mul`/`__dyn_div` (l'opérande à type statique connu F64/Bool est d'abord boxé — jamais taggé nativement, contrairement à la même valeur logée dans un `mixed` — pour rejoindre la même représentation ; un `I64`/`Ptr` est déjà dans cette représentation tel quel) et retournent la valeur "mixed" auto-décrite obtenue, sans la déballer immédiatement.
- `expr_ir_type` (typeinfer.rs) rapporte donc aussi `Ptr` pour `-`/`*`/`/` dès qu'un opérande est `Ptr` (déjà le cas pour `+` depuis toujours) — cohérent avec ce que `lower_expr` produit réellement.
- Le déballage final vers un `int`/`float` concret se fait au point de consommation (`box_for_any`, à l'affectation) ou récursivement si le résultat alimente un autre opérateur arithmétique.

Vérifié par reproduction, avant/après, sur : `int + mixed(int)` (105, plus 1062449896), `mixed(int) - int`/`mixed(int) * int` (déjà corrects, non régressés), `mixed(float) - float`/`float * mixed(float)` (auparavant un nombre dénormalisé proche de zéro, maintenant corrects), `mixed(float) + float` dans les deux ordres avec destination `float` (auparavant un nombre dénormalisé, maintenant correct), `mixed(float) / int` connu — càd sans aucun indice statique float (auparavant tronqué en division entière donnant `0`, maintenant `0.8333333333333334`), affectation directe `mixed→float` sans arithmétique (auparavant faux, maintenant correcte), et non-régression de la concaténation string existante (`"foo"+"bar"`, `mixed(string)+string`, `"texte: "+mixed(int)`). `make regression` sans régression (386 PASS / 0 FAIL, seule l'erreur pré-existante déjà documentée `mainTest.oc`/`Printable` subsiste).

## Limite assumée, restante

Un `mixed` contenant un **float** combiné, dans un `-`/`*`/`/`, à une valeur de type **statiquement connu `int`** (jamais `float`) est maintenant traité correctement (voir ci-dessus — c'était justement le bug intermédiaire découvert et corrigé). La limite qui subsiste est plus étroite : elle touche uniquement `%`, qui reste toujours entier par choix (`Inst::Mod` n'a aucun support flottant dans ce compilateur, indépendamment de `mixed` — un `float % float` connu statiquement est tout aussi peu défini aujourd'hui, hors sujet de ce chantier).

## Fichiers clés

`runtime/src/lib.rs` (`__dyn_add`/`__dyn_sub`/`__dyn_mul`/`__dyn_div`, `__mixed_to_int`/`__mixed_to_float`, `unbox_numeric_i64`/`unbox_numeric_f64`, `is_heap_object`), `src/codegen/desc.d/lowlevel.rs` (signatures), `src/lower/expr.d/lower.rs` (`Expr::Binary`, `box_for_dyn_arith`, `unbox_mixed_operand`), `src/lower/expr.d/typeinfer.rs` (`expr_ir_type` pour `Sub`/`Mul`/`Div`), `src/lower/stmt.d/statements.d/helpers.rs` (`box_for_any`, sens manquant ajouté).
