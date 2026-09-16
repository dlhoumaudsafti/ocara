# Durcir la représentation `mixed` — sortir du régime « un bug trouvé, un correctif »

## ✅ Terminé

Volet 1 (filet de tests systématiques + correctif fuite/aliasing sur les valeurs boxées) fait. Volet 2 (évaluer une représentation moins heuristique) conclu sans réécriture — voir le détail ci-dessous. Ticket retiré de la Priorité Haute de `docs/roadmap.md`.

## Constat

Les huit correctifs listés dans [memoire-fiabilite-runtime-bas-niveau](memoire-fiabilite-runtime-bas-niveau.md) partagent tous la même cause structurelle : la distinction pointeur/valeur d'un `mixed` (`runtime/src/typecheck.rs`) repose sur une heuristique — `val >= PTR_THRESHOLD (65536) && (val & 3) == 0` — pas sur un tag garanti par construction. Chacun a été trouvé par **reproduction manuelle** d'un cas concret (`0`/`null`, `array<float>`, `array<array<float>>`, exception/`map`, décalage sucre/statique, `n is Shape` sur un `int`), jamais par une preuve couvrant toute la classe de bug. Rien ne garantit qu'une neuvième combinaison (ex. un `float` boxé comparé à un `int` boxé dans un contexte encore non exercé, un `mixed` capturé par une closure puis relu après plusieurs niveaux de wrapping) ne reproduira pas exactement le même symptôme.

C'est la définition même d'une dette qui ne se résorbe jamais par construction — seulement par la patience du prochain rapport de bug.

## Deux volets

### 1. ✅ Filet de sécurité — tests systématiques par propriété, pas par cas isolé

Fait — `runtime/src/tests/boxing.rs` (déclaré `#[cfg(test)] mod tests;` depuis `runtime/src/lib.rs`, `runtime/src/tests/mod.rs` fait `mod boxing;` — un fichier par domaine testé, voir docs/roadmap.d/qualite-tests-unitaires-critiques.md, pour ne pas alourdir davantage lib.rs ; 25 tests avec le groupe 5 ci-dessous, `cargo test -p ocara_runtime`, 0 warning sous `RUSTFLAGS="-D warnings"`). Contrairement aux tests de régression `.oc` existants (`37_mixed_large_intTest.oc`, `38_mixed_call_arg_boxingTest.oc`, `41_nested_container_ownershipTest.oc`...), qui fixent chacun une valeur d'entrée précise ayant déjà fait planter le compilateur une fois, ces tests balaient systématiquement l'espace des valeurs frontières :
- Toutes les valeurs remarquables autour de `PTR_THRESHOLD` (`0`, petits entiers bruts, `65535`/`65536`/`65537`, négatifs jusqu'à `i64::MIN`, grands positifs jusqu'à `i64::MAX`) round-trippées à travers `box_int_if_needed` → `get_value_type` → déboxage — et le même round-trip pour `float` (incluant `NaN`/`MIN`/`MAX`/`EPSILON`) et `bool`.
- `array<int>` imbriqué à profondeur 5 (`array<array<array<array<array<int>>>>>`) et un `array<map<string, array<int>>>` mixte, avec des feuilles choisies délibérément "pointer-shaped" (alignées sur 8, `>= PTR_THRESHOLD`) pour qu'une régression vers le chemin générique fasse planter le test plutôt que passer silencieusement — `free`/`clone` `_concrete`.
- Les 6 comparateurs stricts (`__cmp_eq/ne/lt/gt/le/ge_strict`) sur des paires représentatives : brut vs boxé, float boxé vs int brut (coercition numérique), deux strings de contenu identique mais d'adresses différentes, deux objets tas de contenu identique (comparaison par pointeur, pas structurelle — comportement figé), mismatch de type.
- Une string contenant un NUL à chaque position (début, milieu, fin, NUL consécutifs, string réduite à un seul NUL) — round-trip et comparaison par contenu complet, pas tronqué.

Ce que ce filet ne couvre pas encore, à ajouter si un nouveau cas limite est découvert : les fonctions `pub(crate)`/`pub` exposées par `runtime_sdl`/`runtime_tauri` qui manipulent aussi des valeurs `mixed` (hors périmètre du crate `runtime` testé ici).

### 2. ✅ Évaluer une représentation moins heuristique — conclu, pas de réécriture

**Conclusion** : le schéma actuel (tag `00` = pointeur, `01`/`10`/`11` = primitif boxé, seuil `PTR_THRESHOLD`) n'est **pas une heuristique probabiliste** contrairement à ce que ce ticket supposait initialement — il repose sur une vraie garantie du système d'exploitation (Linux/macOS réservent les adresses `< 0x10000` au noyau, `mmap_min_addr` — déjà noté en tête de `runtime/src/lib.rs`) : un pointeur tas réel est *toujours* `>= 0x10000` avec les 2 bits bas à `00`. La seule façon qu'un entier brut soit confondu avec un pointeur est que le compilateur **omette de le boxer** avant de le loger dans un `mixed` — c'est exactement la cause de chacun des huit bugs listés dans `memoire-fiabilite-runtime-bas-niveau.md` : un site de lowering qui ne passait pas par le bon appel de boxing, pas une faille du schéma de tag lui-même.

Ce boxing est centralisé dans exactement 3 fonctions (`box_for_any`, `box_for_dyn_arith`, `box_arg_for_mixed_param` — `src/lower/`), appelées depuis ~14 sites au total dans `src/lower/expr.d/lower.rs` et `src/lower/stmt.d/statements.d/`. C'est un périmètre borné et auditable, pas une surface diffuse.

Les deux options envisagées initialement ont donc été écartées :
- **Boxer systématiquement tout `mixed`** aurait un coût réel (allocation supplémentaire par valeur, non mesuré faute de benchmark) pour fermer un risque qui n'est pas fondamentalement une question de représentation.
- **Un schéma d'entiers taggés universel** est en réalité plus invasif que ce que ce ticket supposait au départ — pas "réserver un bit", mais changer la représentation de TOUS les entiers en contexte `mixed` et toute l'arithmétique dynamique qui les manipule.

**Décidé : pas de réécriture de la représentation.** Le filet de tests du volet 1 (25 tests, `runtime/src/tests/boxing.rs`) couvre les cas frontières connus ; l'audit exhaustif des ~14 sites de boxing (vérifier qu'aucun ne manque à l'appel, dans l'esprit de `qualite-parite-sucre-statique-param-types.md`) reste une piste valable si un nouveau bug de cette famille est découvert, mais n'est pas retenu comme travail bloquant pour la stabilité — pas de nouvelle fiche ouverte pour ça tant qu'aucun cas concret ne l'exige.

#### ✅ Découverte en écrivant le volet 1, corrigée : une cellule boxée (int/float/bool) n'était jamais libérée ni réellement clonée

`__value_free` (`runtime/src/lib.rs`) — point d'entrée unique de la destruction `scoped`/`consumed` — ne reconnaissait que trois cas : string possédée (`is_owned_string`), array (`__is_array`), map (`__is_map`). Les trois passent par `read_tag`, qui **retourne `0` immédiatement pour toute valeur dont les 2 bits bas ne sont pas `00`** (`runtime/src/typecheck.rs`, `read_tag` : `if val < PTR_THRESHOLD || (val & 3) != 0 { return 0; }`) — c'est exactement le cas de toute cellule boxée par `box_int_if_needed`/`__box_float`/`__box_bool` (tags `01`/`10`/`11`). Conséquence : la cellule de 8 octets allouée pour boxer un `int`/`float`/`bool` dans un `mixed` n'était libérée par aucun chemin existant — chaque valeur primitive boxée fuyait inconditionnellement, pas seulement dans un cas limite.

En creusant plus loin, `__value_clone` avait le défaut symétrique et plus grave : pour une cellule boxée, elle retournait `val` tel quel (le même pointeur, pas une copie) — un alias, pas une "copie profonde indépendante" pourtant promise par `docs/EBNF.md` §9.2 pour l'échappement d'une `scoped`. Corriger seulement `__value_free` sans corriger `__value_clone` aurait donc transformé une fuite (inoffensive mais réelle) en use-after-free/double-free dès qu'un `array<mixed>`/`map<K,mixed>` contenant une valeur boxée s'échappe puis que l'original est détruit.

**Corrigé** : ajout de `is_boxed_primitive`/`free_boxed_primitive`/`clone_boxed_primitive` (`runtime/src/lib.rs`), branchés dans `__value_free` (libère réellement la cellule) et `__value_clone` (alloue une copie indépendante bit-à-bit, valide pour les trois tags). Vérifié par 3 nouveaux tests dans `runtime/src/tests/boxing.rs` (groupe 5) : `value_free_frees_a_boxed_primitive_without_crashing`, `value_clone_of_boxed_primitive_is_an_independent_cell` (libère l'original puis relit le clone — c'est exactement le scénario qui aurait planté/corrompu avant ce correctif), `array_clone_then_free_original_leaves_boxed_element_in_clone_intact` (reproduction du scénario réel d'échappement `scoped array<mixed>` via `__array_clone`/`__array_free`). `cargo test -p ocara_runtime` : 25 passed, 0 warning.

Aucun test de non-régression `.oc` ajouté (rien ne plantait avant — c'était une fuite silencieuse, pas un crash reproductible ; voir `docs/roadmap.d/qualite-tests-unitaires-critiques.md` sur l'intérêt des tests Rust ciblés précisément pour ce genre de cas, invisible en boîte noire).

## Priorité / Complexité

**✅ Terminé.** Les deux volets sont clos : le volet 1 (filet de 25 tests + correctif fuite/aliasing sur les valeurs boxées) referme le risque de régression silencieuse sur tous les cas déjà connus ; le volet 2 conclut, après analyse, que le schéma de tag repose sur une garantie OS réelle (pas une heuristique probabiliste) et que le risque résiduel est une question de couverture du boxing côté compilateur (~14 sites, bornés et auditables), pas de représentation — décision prise consciemment de ne pas réécrire le boxing pour l'instant.

## Fichiers clés

`runtime/src/typecheck.rs`, `runtime/src/lib.rs` (`box_int_if_needed`, `get_value_type`, `is_ptr`/`is_float_box`/`is_bool_box`/`is_int_box`, `__value_free`/`__value_clone`, `is_boxed_primitive`/`free_boxed_primitive`/`clone_boxed_primitive`), `runtime/src/tests/boxing.rs` (filet de tests du volet 1), `src/lower/stmt.d/ownership.rs` (`concrete_elem_shape`), `src/lower/expr.d/helpers.rs` (`box_arg_for_mixed_param`), `src/lower/expr.d/lower.rs` (comparaisons `mixed`).
