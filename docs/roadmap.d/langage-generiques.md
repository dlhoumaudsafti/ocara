# Génériques : vérification incomplète

Arité, typage des accès membres, générique en paramètre/champ/valeur de retour, et `implements` sur un `generic` sont corrigés — voir git log pour le détail (E21, `substitute_type_params`, `var_class` pour `Type::Generic`, E09 étendu à `program.generics`).

## Reste à faire : `extends` sur un `generic` n'est jamais vérifié sémantiquement

Découvert en travaillant sur la syntaxe obsolète des exemples : `generic Bag<T> extends BaseResult { ... }` où `BaseResult` **n'existe pas** compile sans la moindre erreur (confirmé sur `examples/generics/test_syntax.oc`, qui fait exactement ça). Contrairement à une classe concrète (`class X extends Y`, où `Y` inconnu est rejeté), aucune vérification équivalente n'existe pour la déclaration `extends` d'un `generic`.

À faire : appliquer au `extends` d'un `generic` la même vérification que pour une classe concrète (le parent doit exister, ce qui implique aussi de décider si l'héritage entre génériques doit être supporté sémantiquement au-delà du parsing, ou seulement validé comme "le nom existe").

## Fichiers clés

`src/sema/typecheck.rs`/`src/main.rs` (vérifications E09/`program.generics`), `examples/generics/test_syntax.oc`.
