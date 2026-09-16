# Parité statique/sucre — le volet type des PARAMÈTRES reste non unifié

## ✅ Terminé

`param_type_for_call_arg`/`param_type_for_sugar_call_arg` unifiées en une seule fonction `param_type_for_call_arg(builder, mangled, i, form: CallForm)` (`src/lower/expr.d/helpers.rs`), même patron que `resolve_method_return_type`. Les 3 sites d'appel (`src/lower/expr.d/lower.rs`) mis à jour (`CallForm::Sugar` pour le sucre d'instance, `CallForm::Static` pour l'appel statique et la fonction libre) — aucun changement de comportement, seulement une source de vérité unique pour le décalage d'index. 4 tests unitaires Rust ajoutés (`src/lower/expr.d/tests.rs`, `cargo test -p ocara --bin ocara lower::expr::tests`, 45 passed, 0 warning). `make regression` : 637 PASS, 0 FAIL, 0 ERREUR — inchangé.

## Constat

`docs/roadmap.d/qualite-parite-sucre-statique.md` (chantier précédent, ✅ terminé) a unifié la résolution du **type de retour** entre forme statique (`Array::get(arr, i)`) et sucre d'instance (`arr.get(i)`) via `resolve_method_return_type` (`src/lower/expr.d/typeinfer.rs`), une seule source de vérité pour les deux formes.

Le volet **type des paramètres** (utilisé pour décider si un argument doit être boxé avant l'appel, voir [memoire-fiabilite-runtime-bas-niveau](memoire-fiabilite-runtime-bas-niveau.md) point 7) n'a, lui, pas été unifié de la même façon : `src/lower/expr.d/helpers.rs` définit toujours **deux fonctions séparées** —
- `param_type_for_call_arg(builder, mangled, i)` (forme statique, index `i` = position réelle incluant le récepteur en 0) ;
- `param_type_for_sugar_call_arg(builder, mangled, i)` (forme sucre, index `i` = position dans les arguments explicites, décalée de `+1` en interne pour compenser le récepteur implicite avant de consulter `builtin_method_param_types()`).

C'est exactement la même structure de risque que celle qui a causé le SEGFAULT documenté dans `docs/roadmap.d/langage-array-get-display-bug.md` : deux fonctions qui encodent la même information sous deux formes légèrement différentes, avec un décalage d'index maintenu à la main plutôt que dérivé automatiquement. Le commentaire de `param_type_for_sugar_call_arg` (`src/lower/expr.d/helpers.rs:90-104`) documente lui-même ce risque en racontant le bug d'origine — mais la duplication qui l'a permis n'a pas été supprimée, seulement corrigée pour le cas trouvé.

Tout futur ajout d'un builtin à double forme (au-delà des 6 déjà couverts — `String`, `Array`, `Map`, `JSON`, `HTTPRequest`, `HTTPResponse`, voir `allows_instance_sugar` dans `src/sema/typecheck.rs`) devra faire fonctionner ces deux fonctions correctement, sans garde-fou structurel autre que la vigilance humaine et les tests de régression — c'est-à-dire exactement les conditions qui ont produit le bug déjà corrigé une fois.

## Ce qui est demandé

Appliquer à `param_type_for_call_arg`/`param_type_for_sugar_call_arg` le même traitement qu'à `resolve_method_return_type` : une seule fonction, avec un paramètre explicite (`is_sugar: bool`, ou directement la forme d'appel `CallForm::Static`/`CallForm::Sugar`) qui centralise le calcul du décalage d'index au lieu de le laisser dupliqué dans deux corps de fonction. Ajouter un test unitaire Rust dédié (voir [qualite-tests-unitaires-critiques](qualite-tests-unitaires-critiques.md), point 3) qui vérifie explicitement le décalage attendu entre les deux formes pour empêcher toute régression silencieuse future.

## Priorité / Complexité

**✅ Terminé.** Était Priorité Haute (même classe de bug qu'un SEGFAULT déjà confirmé en production, chemin de résurgence concret pour tout futur builtin à double forme) — fermé par unification structurelle plutôt que par un nouveau correctif ponctuel, avec filet de tests dédié.

## Fichiers clés

`src/lower/expr.d/helpers.rs` (`param_type_for_call_arg`, `CallForm`, `box_arg_for_mixed_param`), `src/lower/expr.d/lower.rs` (3 sites d'appel), `src/lower/expr.d/tests.rs` (filet de tests, fait), `src/lower/expr.d/typeinfer.rs` (`resolve_method_return_type`, précédent direct suivi), `src/sema/typecheck.rs` (`allows_instance_sugar`), `docs/roadmap.d/qualite-parite-sucre-statique.md`, `docs/roadmap.d/langage-array-get-display-bug.md`.
