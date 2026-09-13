# Stratégie de gestion mémoire pour `var` (le mot-clé par défaut)

## État actuel

`var` — le mot-clé de déclaration par défaut, utilisé 883 fois dans `examples/` contre 359 pour `scoped` et 4 pour `consumed` — **ne déclenche jamais** de libération (`register_owned_local`, src/lower/stmt.d/ownership.rs:94-110, ignore tout ce qui n'est pas `Scoped`/`Consumed`). Toute valeur allouée sur le tas (string, array, map, instance) et stockée dans un `var` fuit inconditionnellement pour toute la durée du programme.

Dans le même esprit, les environnements de closures (variables capturées "heap_promoted") sont alloués via `__alloc_obj` sans jamais être enregistrés dans le mécanisme de libération (`ownership_class` classe `Type::Function` en `Unsupported`, src/sema/scope.rs:90) : **toute closure créée fuit**, indépendamment du mot-clé utilisé pour la variable qui la contient.

C'est une contrainte dure et assumée du projet (pas de GC, jamais), documentée comme un chantier futur non encore adressé — voir [[ocara-language-design-notes]].

## Pourquoi c'est un chantier "massif"

Contrairement aux bugs ponctuels listés dans les autres fiches, il ne s'agit pas de corriger un mécanisme existant mais d'en construire un nouveau : décider d'une stratégie cohérente (comptage de références, région/arène par fonction, une forme d'analyse d'échappement généralisée qui rendrait `scoped` implicite, etc.), l'implémenter à travers `sema` → `lower` → `codegen` → `runtime`, et la documenter. Ça touche potentiellement l'intégralité du pipeline mémoire actuel (`class_ownership.rs`, `ownership.rs`, tout `runtime/src/lib.rs`).

## Dépendances

Cette stratégie doit être pensée en cohérence avec la correction de [l'échappement par argument](memoire-echappement-argument.md) — les deux touchent la même question de fond (qui possède une valeur, jusqu'à quand).

## Fichiers clés

`src/lower/stmt.d/ownership.rs`, `src/lower/builder.d/class_ownership.rs`, `src/sema/scope.rs`, `runtime/src/lib.rs`.
