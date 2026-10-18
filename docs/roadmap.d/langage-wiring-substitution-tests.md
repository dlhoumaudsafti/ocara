# Remplacer un `wiring` depuis un test — fait

Demandé le 2026-10-17, en écrivant les tests de
`examples/advanced/mini_project_hexa` ; fait le 2026-10-18.

## Constat

Un contrat statique résolu par `wiring` (`CarContract::find(id)` dans un cas
d'usage) était lié à son implémentation sans recours possible pour un test :
faire utiliser un faux dépôt demandait de modifier le code de production.

## Ce qui a été tranché

Une instruction de classe, comme dans une interface, réservée aux classes
de test unitaire :

```ocara
class CarDetailsServiceTest extends UnitTest {
    wiring tests.support.fake.FakeCarRepository
    wiring tests.support.fake.FakeMaintenanceRepository
}
```

- **Effet** : la classe visée devient le premier `wiring` de chaque
  interface qu'elle `implements`, pour toute la compilation du test. Tout
  nom nu de ces interfaces (`CarContract::find`, `use CarContract()`) la
  désigne. Le `wiring` vaut import implicite.
- **Alias d'import** : non remplacés (ils nomment explicitement une
  implémentation), pas plus qu'un nom de classe concrète écrit en clair.
- **Portée** : classe `extends UnitTest`, dans le fichier compilé
  `*Test.oc` (dont le fichier généré par `ocaraunit`). Refusé dans un
  fichier importé, une classe ordinaire, un `module` ou un `generic`.
- **Vérifications** : classe visée introuvable, classe qui n'implémente
  aucune interface, interface remplacée par deux classes différentes. La
  conformité du faux au contrat (signatures) reste celle de tout
  `implements`.

Implémentation : `src/core/test_wiring.rs` (contrôles et substitution),
`ClassDecl::wirings`, parseur (`parse_members`), import implicite
(`analysis.d/imports.rs`). Spécification : EBNF §17.2, doc
`docs/builtins/UnitTest.md`.

## Validation

- `examples/tests/94_test_wiringTest.oc`, `src/core/tests_test_wiring.rs`.
- `mini_project_hexa` : les tests de cas d'usage, contrôleurs, routes et
  serveur utilisent les faux dépôts de `tests/support/fake/` (base en
  mémoire `FakeDatabase`) ; seuls les tests des adaptateurs SQL
  (`tests/context/*/infra/db`) touchent encore `app.db`. Le même cas
  d'usage compilé par `main.oc` appelle toujours `CarRepository`
  (vérifié dans l'IR : `CarRepository_find` vs `FakeCarRepository_find`).
