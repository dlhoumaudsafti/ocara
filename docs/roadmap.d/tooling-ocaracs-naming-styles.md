# ocaracs — styles de nommage configurables et indentation — implémenté

Documentation utilisateur : `tools/ocaracs/README.md` (R01, R13 à R20).

## Ce qui a été tranché

- **R13 `naming_const_embed`** (défaut `true`) : une `const` déclarée dans un
  corps est vérifiée avec le style R14. À `false`, elle n'est plus vérifiée du
  tout (elle ne retombe pas sous R09). Sont des corps : fonction, méthode,
  constructeur `init(...)`, closure `nameless`, bloc runtime
  (`init`/`main`/`error`/`success`/`exit { }`), et un fichier runtime entier
  (`*.runtime.oc`, `*.run.oc`, `*.rt.oc`). Les accolades sont comptées hors
  chaînes, hors backticks et hors commentaires (`src/scope.rs`).
- **R14 `naming_const_embed_is`** : par défaut, le style des variables
  (`naming_var_is`, donc `snake_case`). Une valeur explicite l'emporte.
- **R15 à R18** : `naming_const_is`, `naming_var_is`, `naming_function_is`,
  `naming_class_is`. Valeurs possibles : `snake_case`, `camelCase` (strict,
  sans `_`), `PascalCase`, `UPPER_SNAKE_CASE`, `UPPERCASE` (sans `_`). Chaque
  avertissement propose le nom converti (`→ isAdult`).
- **R19/R20** : `indentation_type = auto|space|tab` et `indentation_gap = N`
  (défauts `auto` et `0`, déduits de la première ligne indentée comme avant).
- **Valeur invalide** : signalée au chargement de `.ocaracs`, la valeur par
  défaut est conservée.

## Effet sur le corpus `examples/`

- R09 : les 152 avertissements de constantes concernaient tous des `const`
  locales. Il en reste 4 sous R14 (`maintenanceTotal`, `improvementTotal`,
  `carDetails`, `fileContent`, en camelCase).
- R08 strict : 168 fonctions et méthodes contenant un `_` (`is_adult`,
  `retry_limit_ok`…) sont désormais signalées. Elles étaient tolérées tant que
  R08 ne vérifiait que la première lettre.

## Fichiers clés

`tools/ocaracs/src/main.rs`, `src/config.rs`, `src/naming.rs`, `src/scope.rs`
(tests unitaires : `make tests-tools`), `tools/ocaracs/README.md`,
`docs/conventions.md`.
