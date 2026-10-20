# `Array::join/sort/contains/indexOf`, affichage des conteneurs et `UnitTest` — corrigé

Audit du 2026-10-19, corrigé le 2026-10-19/20.

## Constat

Les fonctions du runtime lisaient les éléments d'un tableau sans tenir compte
de leur genre brut (`int`/`float`/`bool` non boxés) ni du contenu des
chaînes : `Array::join([100000])` → SIGSEGV, `[0, 1]` → `null,1`, tri et
`contains` des chaînes par adresse, tri des flottants sur leurs bits,
`IO::writeln(array)` → octets bruts, assertions `UnitTest` fausses sur
tableaux et flottants.

## Correctif (`runtime/src/lib.rs`)

- `fmt_elem(raw_kind, v)` formate un élément selon le genre du conteneur ;
  `ptr_to_display` affiche tableaux (imbriqués compris) et maps
  (`[1, 0]`, `{a: 1}`) pour `IO::writeln`, les gabarits et `UnitTest`.
- `elem_eq` / `elem_cmp` / `dyn_cmp` : `contains`/`indexOf`/`sort` comparent
  nombres par valeur et chaînes par contenu ; `join` passe par `fmt_elem`.
- `UnitTest` : messages de `assertGreater`/`Less…` lisibles,
  `assertEmpty`/`NotEmpty` sur la longueur des conteneurs, `assertEquals`/
  `NotEquals` comparent tableaux et maps élément par élément
  (`values_deep_eq`). `assertEnum` : vérifié à la compilation (doc corrigée).

## Tests

`examples/tests/97_array_raw_elements_displayTest.oc`.
