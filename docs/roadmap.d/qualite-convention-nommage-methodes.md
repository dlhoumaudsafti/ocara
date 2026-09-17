# Convention de nommage des méthodes utilisateur non tranchée (`snake_case` vs `camelCase`)

## ✅ Terminé

La convention est tranchée et documentée dans [docs/conventions.md](../conventions.md) : variables/propriétés en `snake_case`, constantes en `MAJUSCULES_SOUS_TIRET`, fonctions/méthodes en `camelCase`, classes/interfaces/modules/generics en `PascalCase`.

`ocaracs` porte maintenant ces quatre règles (`tools/ocaracs/src/main.rs`) :
- **R07** étendue de `class` seul à `class`/`interface`/`module`/`generic` (PascalCase).
- **R08** étendue des fonctions libres aux méthodes (`method`, avec visibilité/`static`/`async` optionnels devant — une signature de méthode d'interface n'a pas de visibilité).
- **R09** étendue de `const` global aux constantes de classe (visibilité optionnelle devant `const`).
- **R12** (nouvelle) : `var`/`scoped`/`consumed`/`property` en `snake_case`.

Limite assumée, cohérente avec le reste de l'outil (analyseur ligne à ligne, pas un vrai parseur) : R12 ne couvre pas les paramètres de fonction/méthode ni les variables de boucle `for`. Vérifié par compilation propre (`make build-tools-dev`, 0 warning sous `RUSTFLAGS="-D warnings"`) et test manuel sur un fichier à violations volontaires (les 4 règles se déclenchent correctement, aucun faux positif sur un fichier conforme). Aucune réécriture du corpus d'exemples existant — cohérent avec le point 3 ci-dessous.

## Constat

Les builtins du runtime sont uniformément en `camelCase` (`strToInt`, `tryLock`, `renderCached`...). Mais côté code **utilisateur**, les exemples ne convergent pas vers une seule convention :

- Exemples numérotés plus anciens : `snake_case` quasi systématique — `is_adult`, `note_to_mention` (`examples/04_conditions.oc`), `get_score`/`add_points`/`get_id`/`get_age` (`examples/26_modules.oc`), `is_positive`/`is_valid_age` (`examples/23_static_method.oc`).
- `examples/project/classes/*.oc` : mélange des deux dans les mêmes fichiers (`get_title`/`get_id` en `snake_case` à côté de `display` neutre).
- `examples/advanced/*` (le code le plus récent) : uniformément `camelCase` — `getColor`, `getTitle`, `fetchUserCount`, `updateEstimatedPrice`.

Rien dans le compilateur ni dans `ocaracs` (linter/analyseur de style du projet) ne semble imposer ou même recommander une convention — voir `tools/ocaracs/README.md`. Ce n'est pas un défaut du langage (aucune des deux formes n'est fausse), mais un apprenant qui compare deux exemples pédagogiques n'a aucun signal sur laquelle est "idiomatique" en Ocara.

## Ce qui est demandé

1. Décider d'une convention officielle pour le nommage des méthodes/fonctions utilisateur (`camelCase` recommandé, par cohérence avec les builtins déjà 100% `camelCase`).
2. Vérifier si `ocaracs` peut/doit émettre un avertissement de style sur les méthodes `snake_case` (à la manière d'un linter classique), sans le rendre bloquant.
3. Une fois la convention actée, ce n'est **pas** un ticket "réécrire tous les exemples en masse" — mais toute nouvelle doc/exemple écrite après ce ticket doit suivre la convention choisie.

## Priorité / Complexité

**Terminé.** Était Priorité Très Basse, Complexité Légère — confirmé : décision + 4 règles de lint, aucun changement de compilateur.

## Fichiers clés

[docs/conventions.md](../conventions.md) (décision, source de vérité), `tools/ocaracs/src/main.rs` (règles R07/R08/R09/R12), `tools/ocaracs/README.md` (documentation des règles).
