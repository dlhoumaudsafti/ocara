# Zéro test Rust unitaire hors lexer/parser — les zones les plus dangereuses sont les moins testées

## Constat

Sur 41 `#[test]` dans tout `src/` (168 fichiers, 25 902 lignes), 100 % se trouvent dans deux fichiers : le lexer et le parser (`src/parsing/lexer.d/tests.rs`, `src/parsing/parser.d/tests.rs`). **Aucun test Rust unitaire** n'existe pour :
- `src/sema/escape.rs` (analyse d'échappement interprocédurale, point fixe conservateur — `compute_escaping_params`) ;
- `src/lower/stmt.d/ownership.rs` (insertion des drops/clones, `emit_scope_drops`, `drop_consumed_used_in`, `concrete_elem_shape`) ;
- `src/sema/typecheck.rs` (2091 lignes, le plus gros fichier du projet — vérification de types, résolution `mixed`) ;
- `runtime/` dans son ensemble (3695 lignes, 489 `unsafe`, 0 test — boxing, `alloc_str`/`free_str`, comparateurs `mixed`).

La fiabilité de tout le pipeline sema→lower→codegen repose aujourd'hui entièrement sur la suite de régression boîte noire (`ci/regression.sh`, 203 fichiers `.oc`, plus `ci/unittests.sh` via `ocaraunit`) : on valide l'ownership/le boxing/le codegen en compilant et exécutant des programmes complets, jamais par un test ciblé sur une fonction précise. Conséquence concrète déjà vécue plusieurs fois (voir `docs/roadmap.d/langage-array-get-display-bug.md`, `memoire-fiabilite-runtime-bas-niveau.md`) : un bug dans une fonction pure comme `param_type_for_sugar_call_arg` ou `box_int_if_needed` n'est détecté qu'au bout de la chaîne — via un SEGFAULT reproduit en compilant un programme `.oc` entier — jamais par un test qui isole directement la fonction fautive.

## Pourquoi c'est un vrai problème de fiabilité, pas seulement de style

Un test de régression boîte noire dit "quelque chose s'est cassé quelque part dans le pipeline" ; il ne dit jamais "c'est `compute_escaping_params` qui a régressé sur tel cas précis". Beaucoup des fonctions concernées sont **pures et directement testables en isolation** sans passer par tout le pipeline (`is_ptr`/`is_float_box`/`is_bool_box`/`is_int_box`, `concrete_elem_shape`, `is_concrete_primitive_elem`, `param_type_for_call_arg`/`param_type_for_sugar_call_arg`, `box_int_if_needed`) — un test unitaire dessus donnerait un diagnostic immédiat et localisé au lieu d'un SEGFAULT découvert des dizaines de commits plus tard sur un exemple sans rapport apparent.

## Ce qui est demandé

Ne pas viser une couverture exhaustive d'un coup — prioriser les fonctions déjà identifiées comme sources historiques de bugs (voir [memoire-fiabilite-runtime-bas-niveau](memoire-fiabilite-runtime-bas-niveau.md) et [memoire-boxing-durcissement](memoire-boxing-durcissement.md) pour le détail des cas limites à couvrir côté boxing) :
1. `runtime/` : premier `#[test]` du crate, sur les fonctions de tag/boxing (`is_ptr`, `is_float_box`, `is_bool_box`, `is_int_box`, `box_int_if_needed`, `get_value_type`) — aucune dépendance sur le compilateur, testable immédiatement.
2. `src/lower/stmt.d/ownership.rs` : `concrete_elem_shape`/`is_concrete_primitive_elem` sur des `Type` construits à la main (array/map imbriqués à profondeur variable).
3. `src/lower/expr.d/helpers.rs` : `param_type_for_call_arg` vs `param_type_for_sugar_call_arg` — un test qui vérifie explicitement le décalage `+1` attendu entre les deux, pour empêcher une régression silencieuse de la même classe que celle documentée dans `langage-array-get-display-bug.md`. Rejoint directement [qualite-parite-sucre-statique-param-types](qualite-parite-sucre-statique-param-types.md).
4. `src/sema/escape.rs` : `compute_escaping_params` sur quelques graphes d'appel synthétiques (fonction qui retient son paramètre, fonction qui ne fait que le lire, cas non résolu).

## Priorité / Complexité

**Priorité Haute** — condition nécessaire pour pouvoir affirmer une fiabilité durable plutôt qu'un historique de correctifs réactifs : sans ça, chaque futur ajout au boxing/à l'ownership repart avec le même risque de découverte tardive par SEGFAULT. **Complexité : Structurel** — pas un gros morceau technique unitairement, mais demande de traverser plusieurs modules et d'établir un premier harnais `#[test]` dans `runtime/` qui n'existe pas du tout aujourd'hui.

## Fichiers clés

`runtime/src/lib.rs`, `runtime/src/typecheck.rs`, `src/sema/escape.rs`, `src/lower/stmt.d/ownership.rs`, `src/lower/expr.d/helpers.rs`, `src/parsing/lexer.d/tests.rs` et `src/parsing/parser.d/tests.rs` (comme référence de style existant à suivre).
