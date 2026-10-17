# `make ci` rouge : exemples non conformes au linter et à la couverture

Statut : **à corriger** — constaté le 2026-10-17, une fois les échecs de
`ci/start.sh` rendus visibles (tests manquants de 33/70 ajoutés, `$OCARAUNIT
--clear` corrigé, codes de retour agrégés).

## Constat

`make ci` échoue sur 29 des 44 exemples `examples/NN_*.oc` (détail dans
`ci/ci-report.txt` après un `make ci`) :

- **Linter (19 exemples)** — `ocaracs` signale des avertissements de style :
  fonctions en snake_case au lieu de camelCase (`04_conditions`,
  `30_variadic`…), variables en camelCase au lieu de snake_case
  (`32_strict_operators`, `60_interface_wiring`…), fichier sans newline finale.
- **Couverture (18 exemples)** — `ci/start.sh` exige 100 % de fonctions
  couvertes (une méthode `<fonction>Test` par fonction) ; la plupart sont à
  0 % faute de tests nommés d'après les fonctions de l'exemple.

## Correction attendue

Renommer les identifiants dans les exemples (et leurs tests, et la
documentation qui les cite), puis compléter les tests `examples/tests/NN_*Test.oc`
avec une méthode `<fonction>Test` par fonction. Alternative à trancher :
assouplir l'étape 4 de `ci/start.sh` (seuil de couverture) si 100 % est jugé
excessif pour des exemples pédagogiques.

## Validation

`make ci` termine en `✓ Pipeline CI : SUCCÈS`.

## Priorité / Complexité

**Moyenne** (le pipeline CI ne peut pas servir de garde tant qu'il est
rouge par construction) — **Légère** (volume, sans difficulté technique).
