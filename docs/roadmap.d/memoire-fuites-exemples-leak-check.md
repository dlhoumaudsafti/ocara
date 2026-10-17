# Fuites rapportées par `--leak-check` sur les exemples

Statut : **à corriger** — constaté le 2026-10-17, à la mise en place de
`ci/leak_check.sh` (jusque-là, `--leak-check` n'était lancé nulle part).

## Constat

`make leak-check` compile chaque `examples/NN_*.oc` avec `--leak-check`.
38 exemples sont propres ; 5 fuient. Ils sont listés dans `KNOWN_LEAKS`
(`ci/leak_check.sh`) pour que la passe reste verte sur ces fuites connues
tout en échouant sur toute fuite nouvelle :

| Exemple | Fuite | Site |
|---|---|---|
| `20_try_fail` | 1 × `string` | `open_file` l.39 — message `"…" + path` d'une exception levée |
| `60_interface_wiring` | 4 × `string` | `PostgresRepo.save` l.40, `InMemoryRepo.save` l.49 — retour `` `…${msg}` `` par dispatch d'interface |
| `64_async_instance_method_dispatch` | `Dog`, `Cat`, `Robot`, `ConcreteRepo` | `main` l.67, `pickTalker` l.59 — receveurs d'appels `async` sur méthode d'instance |
| `65_async_resolvable_return_type` | 1 × `Doubler` | `main` l.36 — receveur d'un appel `async` |
| `67_named_arguments` | 1 × `string` | `AdminDto.describe` |

Pistes : les receveurs d'appels `async` (64, 65) ne semblent jamais relâchés
par la tâche ; les chaînes retournées via dispatch d'interface (60) et le
message d'une exception construite par concaténation (20) restent à
investiguer.

## Correction attendue

Corriger chaque fuite puis retirer l'exemple de `KNOWN_LEAKS` — la passe
échoue d'elle-même quand un exemple listé ne fuit plus.

## Validation

`make leak-check <exemple>` affiche `OK` pour chaque exemple corrigé ;
`make leak-check` passe avec `KNOWN_LEAKS` vide.

## Priorité / Complexité

**Haute** (fuites mémoire) — **Légère** à **Structurel** selon la cause
(async : probablement dans la gestion de propriété des receveurs de tâches).
