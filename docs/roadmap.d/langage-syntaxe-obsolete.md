# Syntaxe obsolète et décalages doc/exemples

- **`T[]` au lieu de `array<T>`** : syntaxe non supportée par le parser actuel (confirmé par compilation : `error: expected visibility... found LBracket`) mais encore présente dans plusieurs fichiers, qui ne compilent donc plus (`examples/generics/List.oc`, `examples/generics/test_syntax.oc`, `examples/runtime/variables.oc`, plusieurs `examples/tests/*.oc`).
- **Opérateur `==`** (remplacé par `equal` depuis la v0.2.0) encore présent dans 2 fichiers d'exemple.
- **Contradiction namespace imbriqué** : l'EBNF affirme que `namespace utils.http` n'est "pas supporté pour l'instant", alors que 28 fichiers de `examples/advanced/` utilisent des namespaces imbriqués sans problème — c'est la doc qui est fausse, pas le compilateur.
- **Combinaisons génériques non illustrées** : `extends`+`implements`+`modules` combinés sur un `generic`, `Option<T>`, `QualifiedType` en annotation de type — documentés dans l'EBNF mais sans aucun exemple réel dans le dépôt.
- 20 fichiers `.bak` dans `examples/tests/` : artefacts de la migration `==`/`<`/`>` → `equal`/`smaller`/`greater`, à nettoyer.

## Ampleur

Correctifs simples et mécaniques (mettre à jour les exemples, corriger la phrase sur les namespaces dans l'EBNF, ajouter quelques exemples de combinaisons génériques, supprimer les `.bak`) — aucun ne demande de changement de compilateur.

## Fichiers clés

`examples/generics/`, `examples/tests/`, `docs/EBNF.md` (§3 namespaces).
