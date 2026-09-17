# Réflexion — remplacer `for x in a..b` par `for x in a to b` / `for x in a until b`

## Statut

**Réflexion ouverte, pas une décision.** Ce ticket documente une piste d'évolution de syntaxe à peser, pas un chantier acté. Objectif de cette fiche : que la discussion et ses compromis ne se perdent pas, jusqu'à ce qu'une décision explicite soit prise (adopter, adapter, ou classer sans suite).

## Proposition

Aujourd'hui, une plage s'écrit `a..b` avec borne de fin **exclue** (`RangeExpr ::= AdditiveExpr ".." AdditiveExpr`, `docs/EBNF.md:3179` ; `for j in 1..6` parcourt `1,2,3,4,5` — `examples/07_loops.oc:25-27`). C'est du C#/Rust `..`/`..=`-like, mais l'exclusivité de la borne de fin n'est pas lisible au point d'appel : il faut connaître la convention pour savoir que `1..6` s'arrête à 5, pas à 6.

Piste proposée : remplacer (ou compléter) `..` par deux mots-clés explicites :
- `for x in 1 to 10` → borne de fin **incluse** (1 à 10, 10 compris)
- `for x in 1 until 10` → borne de fin **exclue** (1 à 9, comportement actuel de `1..10`)

L'intérêt : le nom de l'opérateur porte lui-même la sémantique de la borne, ce qui supprime une classe d'erreurs "off-by-one" à la lecture — cohérent avec la philosophie déjà à l'œuvre dans le choix des opérateurs de comparaison en toutes lettres (`equal`, `smaller or equal`...) plutôt que des symboles ambigus, voir `examples/32_strict_operators.oc`.

## Ce qu'il faut pester avant de trancher

**Pour :**
- Cohérence avec le parti pris général du langage : lisibilité explicite plutôt que symboles courts (voir la suppression complète des opérateurs de comparaison symboliques en v0.2.0).
- Élimine une confusion réelle et déjà documentée : plusieurs exemples commentent explicitement "end exclus" à côté de chaque usage de `..` (`examples/07_loops.oc:9`, `examples/15_operators.oc:63`) précisément parce que ce n'est pas déductible de la syntaxe elle-même.

**Contre / risques :**
- **Changement de syntaxe cassant** : `..` est utilisé dans une bonne partie du corpus d'exemples existant (`examples/07_loops.oc`, `08_arrays.oc` via boucles, `14_static_access.oc`, `15_operators.oc`, `19_break_continue.oc`, `23_static_method.oc`, et largement dans `advanced/`) — remplacer purement et simplement `..` casserait tout code existant sans période de transition.
- Si les deux formes (`..` et `to`/`until`) coexistent durablement, c'est une divergence stylistique de plus dans un langage qui vient de finir un chantier de convergence documentaire ([coherence-documentation-ebnf-stdlib](coherence-documentation-ebnf-stdlib.md)) — deux façons d'écrire la même chose, dont une seule serait "la bonne", est justement le genre d'ambiguïté que ce chantier a cherché à éliminer ailleurs.
- Portée non triviale : `RangeExpr` est référencée depuis au moins 3 endroits distincts de la grammaire (`docs/EBNF.md:1315`, `:3179`, `:3576`) plus son usage dans `ForStmt`/`is`-narrowing — un changement doit être répercuté aux 3 endroits (voir la leçon de [coherence-documentation-ebnf-stdlib](coherence-documentation-ebnf-stdlib.md) sur le §31 qui dérive silencieusement si une règle n'est modifiée qu'à un seul endroit).
- `to`/`until` sont des mots relativement communs — vérifier qu'ils ne collisionnent avec aucun identifiant déjà utilisé comme nom de variable/méthode dans le corpus existant avant d'en faire des mots-clés réservés.

## Si la piste est retenue

Ne pas supprimer `..` immédiatement : introduire `to`/`until` en parallèle, marquer `..` déprécié (avertissement, pas erreur) le temps d'une période de transition, migrer le corpus d'exemples, seulement ensuite envisager le retrait de `..`. Traiter comme un changement de langage à part entière : mise à jour EBNF (les 3 occurrences de `RangeExpr`, dont impérativement le §31 "grammaire complète"), `src/parsing/`, tous les exemples utilisant `..`, extension VSCode (coloration + complétion), et un test de régression dédié aux deux bornes (incluse/exclue) pour chaque nouveau mot-clé.

## Priorité / Complexité

**Priorité Basse** — confort/lisibilité, portée future ; le comportement actuel de `..` est correct et déjà documenté (juste peu lisible au point d'appel). **Complexité Structurel** si la piste est retenue — touche grammaire, parser, tous les exemples utilisant des plages, et l'outillage (extension VSCode). Ne pas engager tant que la réflexion ci-dessus n'a pas été tranchée explicitement.

## Fichiers clés

`docs/EBNF.md:1315,3179,3576` (`RangeExpr`), `src/parsing/` (lexer/parser de `..`), tout le corpus `examples/` utilisant `..`, `tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`.
