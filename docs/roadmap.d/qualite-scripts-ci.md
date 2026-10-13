# Scripts de CI et cibles de test : échecs masqués

Statut : **à corriger** — constaté le 2026-10-13 (audit de la roadmap).

## Constat

- `make tests-examples` : seul le code de retour du dernier `ocaraunit` compte
  (`examples/project/tests`) — un échec dans `examples/tests` est masqué.
- `make ci` échoue par construction : `ci/start.sh` cherche `21_errorsTest`,
  `33_increment_decrementTest` et `70_whenTest`, qui n'existent pas.
- `ci/start.sh` (~l.238) : `OCARAUNIT --clear` sans `$` (commande inexistante),
  erreur masquée par une redirection.
- `ci/regression.sh` en mode cible unique : avec `set -e`, sortie en 1 sans
  message (simulé, non exécuté sur le vrai script).
- `ci/regression.sh` ne vérifie que le code de sortie des `examples/NN_*.oc`,
  jamais leur sortie ; `--leak-check` n'est appelé nulle part (les fuites
  [memoire-boxing-temporaires-non-relaches](memoire-boxing-temporaires-non-relaches.md)
  et [memoire-system-os-fuite](memoire-system-os-fuite.md) passent inaperçues).

## Correction attendue

Agréger les codes de retour dans `tests-examples` ; corriger la liste de
`ci/start.sh` et le `$` manquant ; message explicite en mode cible unique ;
envisager une sortie attendue (`.out`) pour les exemples et un passage
`--leak-check` sur une sélection d'exemples.

## Validation

Faire échouer volontairement un test dans `examples/tests` et vérifier que
`make tests-examples` et `make ci` échouent.

## Priorité / Complexité

**Moyenne** (fiabilité de la détection des régressions) — **Simple**.
