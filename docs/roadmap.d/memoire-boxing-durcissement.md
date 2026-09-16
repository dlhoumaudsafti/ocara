# Durcir la représentation `mixed` — sortir du régime « un bug trouvé, un correctif »

## Constat

Les huit correctifs listés dans [memoire-fiabilite-runtime-bas-niveau](memoire-fiabilite-runtime-bas-niveau.md) partagent tous la même cause structurelle : la distinction pointeur/valeur d'un `mixed` (`runtime/src/typecheck.rs`) repose sur une heuristique — `val >= PTR_THRESHOLD (65536) && (val & 3) == 0` — pas sur un tag garanti par construction. Chacun a été trouvé par **reproduction manuelle** d'un cas concret (`0`/`null`, `array<float>`, `array<array<float>>`, exception/`map`, décalage sucre/statique, `n is Shape` sur un `int`), jamais par une preuve couvrant toute la classe de bug. Rien ne garantit qu'une neuvième combinaison (ex. un `float` boxé comparé à un `int` boxé dans un contexte encore non exercé, un `mixed` capturé par une closure puis relu après plusieurs niveaux de wrapping) ne reproduira pas exactement le même symptôme.

C'est la définition même d'une dette qui ne se résorbe jamais par construction — seulement par la patience du prochain rapport de bug.

## Deux volets

### 1. Filet de sécurité — tests systématiques par propriété, pas par cas isolé

Aujourd'hui chaque test de régression (`37_mixed_large_intTest.oc`, `38_mixed_call_arg_boxingTest.oc`, `41_nested_container_ownershipTest.oc`...) fixe une valeur d'entrée précise ayant déjà fait planter le compilateur une fois. Ajouter, côté `runtime/` (Rust, `#[test]`, pas `.oc` — voir aussi [qualite-tests-unitaires-critiques](qualite-tests-unitaires-critiques.md)), des tests qui balaient systématiquement l'espace des valeurs frontières plutôt qu'un point unique :
- Toutes les valeurs remarquables autour de `PTR_THRESHOLD` (`0`, `1`, `PTR_THRESHOLD - 1`, `PTR_THRESHOLD`, `PTR_THRESHOLD + 1`, un entier négatif, `i64::MAX`/`MIN`) round-trippées à travers `box_int_if_needed` → `get_value_type` → déboxage, pour `int`/`float`/`bool`.
- `array<T>`/`map<K,T>` imbriqués jusqu'à profondeur 4-5 (pas seulement 2), avec un mélange de types concrets et `mixed` à différents niveaux.
- Toute paire d'opérandes `mixed` × `mixed`, `mixed` × concret, sur les 6 comparateurs (`equal`/`not equal`/`smaller`/`greater`/`smaller or equal`/`greater or equal`), croisée avec les 4 tags (ptr/float/bool/int boxé).
- Une string contenant un NUL à chaque position possible (début, milieu, fin, NUL consécutifs).

### 2. Évaluer une représentation moins heuristique (sans nécessairement la construire maintenant)

Étudier, en gardant à l'esprit la contrainte no-GC du projet, si une représentation alternative éliminerait la classe de bug plutôt que de la rendre seulement moins probable :
- Boxer **systématiquement** tout `mixed` non-pointeur (renoncer à l'optimisation "petit entier brut non boxé") — supprime l'ambiguïté par construction, au prix d'une allocation heap supplémentaire pour chaque `int`/`bool`/`float` logé dans un `mixed`. Mesurer le coût réel avant de trancher (aucun benchmark n'existe aujourd'hui, voir `docs/roadmap.md`).
- Réserver un bit de tag supplémentaire pour distinguer sans ambiguïté "petit entier brut" d'un pointeur, plutôt qu'un seuil de magnitude — change le layout mémoire (`Cargo.lock`/`runtime/src/typecheck.rs`), impact large.

Ce volet est une étude, pas un engagement à réécrire le boxing — la conclusion peut légitimement être "heuristique + volet 1 (tests systématiques) suffit", mais cette décision doit être prise consciemment plutôt que par défaut.

## Priorité / Complexité

**Priorité Haute** — c'est la cause racine commune à tous les SEGFAULTs mémoire confirmés du projet à ce jour ; tant que ce chantier n'est pas traité (au moins le volet 1), le langage ne peut pas être qualifié de stable sur son point le plus sensible. **Complexité : Dangereuse** — touche le cœur du runtime, à traiter avec un budget de tests de non-régression large avant tout changement de comportement (volet 2 en particulier).

## Fichiers clés

`runtime/src/typecheck.rs`, `runtime/src/lib.rs` (`box_int_if_needed`, `get_value_type`, `is_ptr`/`is_float_box`/`is_bool_box`/`is_int_box`), `src/lower/stmt.d/ownership.rs` (`concrete_elem_shape`), `src/lower/expr.d/helpers.rs` (`box_arg_for_mixed_param`), `src/lower/expr.d/lower.rs` (comparaisons `mixed`).
