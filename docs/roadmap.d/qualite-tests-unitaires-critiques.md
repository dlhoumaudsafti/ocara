# Zéro test Rust unitaire hors lexer/parser — les zones les plus dangereuses sont les moins testées

## ✅ Terminé

Les 4 points sont faits. Ticket retiré de la Priorité Haute de `docs/roadmap.md`.

## Constat

Sur 41 `#[test]` dans tout `src/` (168 fichiers, 25 902 lignes), 100 % se trouvent dans deux fichiers : le lexer et le parser (`src/parsing/lexer.d/tests.rs`, `src/parsing/parser.d/tests.rs`). **Aucun test Rust unitaire** n'existe pour :
- `src/sema/escape.rs` (analyse d'échappement interprocédurale, point fixe conservateur — `compute_escaping_params`) ;
- `src/lower/stmt.d/ownership.rs` (insertion des drops/clones, `emit_scope_drops`, `drop_consumed_used_in`, `concrete_elem_shape`) ;
- `src/sema/typecheck.rs` (2091 lignes, le plus gros fichier du projet — vérification de types, résolution `mixed`) ;
- `runtime/` dans son ensemble (3695 lignes, 489 `unsafe`, 0 test — boxing, `alloc_str`/`free_str`, comparateurs `mixed`).

La fiabilité de tout le pipeline sema→lower→codegen repose aujourd'hui entièrement sur la suite de régression boîte noire (`ci/regression.sh`, 203 fichiers `.oc`, plus `ci/unittests.sh` via `ocaraunit`) : on valide l'ownership/le boxing/le codegen en compilant et exécutant des programmes complets, jamais par un test ciblé sur une fonction précise. Conséquence concrète déjà vécue plusieurs fois (voir `docs/roadmap.d/langage-array-get-display-bug.md`, `memoire-fiabilite-runtime-bas-niveau.md`) : un bug dans une fonction pure comme `param_type_for_call_arg` ou `box_int_if_needed` n'est détecté qu'au bout de la chaîne — via un SEGFAULT reproduit en compilant un programme `.oc` entier — jamais par un test qui isole directement la fonction fautive.

## Pourquoi c'est un vrai problème de fiabilité, pas seulement de style

Un test de régression boîte noire dit "quelque chose s'est cassé quelque part dans le pipeline" ; il ne dit jamais "c'est `compute_escaping_params` qui a régressé sur tel cas précis". Beaucoup des fonctions concernées sont **pures et directement testables en isolation** sans passer par tout le pipeline (`is_ptr`/`is_float_box`/`is_bool_box`/`is_int_box`, `concrete_elem_shape`, `is_concrete_primitive_elem`, `param_type_for_call_arg`, `box_int_if_needed`) — un test unitaire dessus donnerait un diagnostic immédiat et localisé au lieu d'un SEGFAULT découvert des dizaines de commits plus tard sur un exemple sans rapport apparent.

## Ce qui est demandé

Ne pas viser une couverture exhaustive d'un coup — prioriser les fonctions déjà identifiées comme sources historiques de bugs (voir [memoire-fiabilite-runtime-bas-niveau](memoire-fiabilite-runtime-bas-niveau.md) et [memoire-boxing-durcissement](memoire-boxing-durcissement.md) pour le détail des cas limites à couvrir côté boxing) :
1. ✅ **Fait.** `runtime/` : premier `#[test]` du crate, sur les fonctions de tag/boxing (`is_ptr`, `is_float_box`, `is_bool_box`, `is_int_box`, `box_int_if_needed`, `get_value_type`, `__value_free`/`__value_clone`) — 25 tests dans `runtime/src/tests/boxing.rs`, voir [memoire-boxing-durcissement](memoire-boxing-durcissement.md). Établit au passage la convention à suivre pour les points 2-4 : un fichier par domaine testé sous un sous-dossier `tests/` (`tests/mod.rs` + `tests/<domaine>.rs`, `use crate::*;`), plutôt que d'alourdir le fichier principal — voir `src/parsing/lexer.d/mod.rs`/`tests.rs` pour l'équivalent déjà existant côté compilateur (`src/`), à mirorer côté `runtime/`.
2. ✅ **Fait.** `src/lower/stmt.d/ownership.rs` : `is_concrete_primitive_elem`/`concrete_elem_shape` relevées à `pub(crate)` (uniquement pour la testabilité, aucun changement de comportement) — 6 tests dans `src/lower/stmt.d/tests.rs` (même patron flat que le point 3 : `stmt.d/` est déjà un dossier de domaine). Couvre notamment la profondeur 5 (`array<array<array<array<array<int>>>>>`) et le mélange array/map (`array<map<string, array<int>>>`), symétriques aux tests runtime du groupe 2 de `runtime/src/tests/boxing.rs` — et surtout le cas qui a un historique réel de bug : `None` dès qu'un type non concret apparaît, pas seulement au premier niveau imbriqué mais à N'IMPORTE QUELLE profondeur.
3. ✅ **Fait.** `src/lower/expr.d/helpers.rs` : `param_type_for_call_arg`/`CallForm` — 4 tests dans `src/lower/expr.d/tests.rs` (`#[cfg(test)] mod tests;` déclaré dans `src/lower/expr.d/mod.rs`, même patron flat que `src/parsing/lexer.d/tests.rs`, pas le sous-dossier `tests/` de `runtime/` — voir la note du point 1 : `expr.d/` est déjà lui-même un dossier de domaine, contrairement à `runtime/src/` qui n'en a pas). Vérifie explicitement le décalage `+1` attendu entre les deux formes sur un cas réel (`Array::get`) et sur des tables utilisateur synthétiques. Fait dans le cadre de [qualite-parite-sucre-statique-param-types](qualite-parite-sucre-statique-param-types.md), maintenant clos.
4. ✅ **Fait.** `src/sema/escape.rs` : `compute_escaping_params`/`var_never_escapes` — 5 tests dans `src/sema/tests/escape.rs` (`src/sema/` n'a pas de dossiers `.d/` par domaine comme `lower/`, donc sous-dossier `tests/` comme pour `runtime/` — `#[cfg(test)] mod tests;` déclaré dans `src/sema/mod.rs`). Graphes d'appel synthétiques construits directement en AST (pas de parsing) : un paramètre de constructeur affecté à un champ (échappe), un paramètre reçu seulement comme récepteur de méthode (n'échappe pas), et — le plus utile — l'asymétrie strict/non-strict documentée dans l'en-tête du module sur un même appel `Array::push(arr, x)` non résolu : `compute_escaping_params` (mode non strict, E26) ne le détecte pas, `var_never_escapes` (mode strict, libération auto d'un `var`) le détecte — les deux comportements sont maintenant figés par un test chacun, pour qu'un futur correctif qui romprait cette asymétrie volontaire échoue immédiatement.

## Priorité / Complexité

**✅ Terminé.** Les 4 points sont faits — 40 tests Rust ajoutés au total sur ce chantier (25 boxing + 4 sucre/statique + 6 ownership + 5 escape), 0 en dehors du lexer/parser avant. Deux patrons de fichier dédié sont établis et documentés (`runtime/src/tests/<domaine>.rs`/`src/sema/tests/<domaine>.rs` pour un crate/module sans dossiers `.d/` par domaine, `<domaine>.d/tests.rs` pour un module qui en a déjà) — à réutiliser pour toute future couverture de test Rust plutôt que d'improviser un nouveau patron. `cargo test -p ocara --bin ocara` : 56 passed. `make regression` : 637 PASS, 0 FAIL, 0 ERREUR.

## Fichiers clés

`runtime/src/lib.rs`, `runtime/src/tests/boxing.rs` (fait), `runtime/src/typecheck.rs`, `src/sema/escape.rs`, `src/sema/tests/escape.rs` (fait), `src/lower/stmt.d/ownership.rs`, `src/lower/stmt.d/tests.rs` (fait), `src/lower/expr.d/helpers.rs`, `src/lower/expr.d/tests.rs` (fait), `src/parsing/lexer.d/tests.rs` et `src/parsing/parser.d/tests.rs` (référence de style déjà existante côté compilateur).
