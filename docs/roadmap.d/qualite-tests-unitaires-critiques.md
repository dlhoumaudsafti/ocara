# Zéro test Rust unitaire hors lexer/parser — les zones les plus dangereuses sont les moins testées

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
2. `src/lower/stmt.d/ownership.rs` : `concrete_elem_shape`/`is_concrete_primitive_elem` sur des `Type` construits à la main (array/map imbriqués à profondeur variable).
3. ✅ **Fait.** `src/lower/expr.d/helpers.rs` : `param_type_for_call_arg`/`CallForm` — 4 tests dans `src/lower/expr.d/tests.rs` (`#[cfg(test)] mod tests;` déclaré dans `src/lower/expr.d/mod.rs`, même patron flat que `src/parsing/lexer.d/tests.rs`, pas le sous-dossier `tests/` de `runtime/` — voir la note du point 1 : `expr.d/` est déjà lui-même un dossier de domaine, contrairement à `runtime/src/` qui n'en a pas). Vérifie explicitement le décalage `+1` attendu entre les deux formes sur un cas réel (`Array::get`) et sur des tables utilisateur synthétiques. Fait dans le cadre de [qualite-parite-sucre-statique-param-types](qualite-parite-sucre-statique-param-types.md), maintenant clos.
4. `src/sema/escape.rs` : `compute_escaping_params` sur quelques graphes d'appel synthétiques (fonction qui retient son paramètre, fonction qui ne fait que le lire, cas non résolu).

## Priorité / Complexité

**Priorité Haute — points 1 et 3 faits, points 2 et 4 restants.** Condition nécessaire pour pouvoir affirmer une fiabilité durable plutôt qu'un historique de correctifs réactifs : sans ça, chaque futur ajout au boxing/à l'ownership repart avec le même risque de découverte tardive par SEGFAULT. **Complexité : Structurel** — pas un gros morceau technique unitairement, mais demande de traverser plusieurs modules ; deux patrons de fichier dédié sont maintenant établis (`runtime/src/tests/<domaine>.rs` pour un crate sans dossiers de domaine, `<domaine>.d/tests.rs` pour un crate qui en a déjà — voir point 3).

## Fichiers clés

`runtime/src/lib.rs`, `runtime/src/tests/boxing.rs` (fait), `runtime/src/typecheck.rs`, `src/sema/escape.rs`, `src/lower/stmt.d/ownership.rs`, `src/lower/expr.d/helpers.rs`, `src/lower/expr.d/tests.rs` (fait), `src/parsing/lexer.d/tests.rs` et `src/parsing/parser.d/tests.rs` (référence de style déjà existante côté compilateur).
