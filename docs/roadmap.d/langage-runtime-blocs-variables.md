# Blocs runtime : variables de `init`/`main` libérées avant `success`/`error`/`exit` — corrigé

Audit du 2026-10-19, corrigé le même jour.

## Constat

EBNF §5.3 : « les variables déclarées dans un bloc sont accessibles dans tous
les blocs suivants ». Or un objet de `main` lu dans `exit` faisait un
SIGSEGV, une chaîne ou un tableau valait `null` (l'exemple de l'EBNF §5.7
affichait `Compteur: null`), et une ressource native ouverte dans `main` et
fermée dans `exit` n'était jamais libérée (`examples/builtins/sqlite.oc`).

## Cause

`lower_runtime_main_manual` (`src/lower/builder.d/runtime.rs`) abaissait
`init` + `main` comme un bloc à part : ses locales étaient relâchées en
sortie de bloc, avant `error`/`success`/`exit`.

## Correctif

- `lower_block` est découpé en `enter_block` / `lower_block_stmts` /
  `leave_block` (`src/lower/stmt.d/block.rs`). Le `main` runtime ouvre UNE
  portée pour `init`, `main`, le `if` des blocs `error`/`success` et `exit` ;
  le `return ERROR` final la libère.
- Un `result` de `main` ne libère que les portées plus internes avant de
  sauter au label de sortie (`builder.runtime_scope`,
  `src/lower/stmt.d/statements.rs`).
- Une déclaration de premier niveau d'`init`/`main` sautée par un `result`
  garde un slot mis à 0 en tête de fonction (`preset_runtime_slots`,
  `runtime_preslots` repris par `declare_local`) : sa libération finale est
  sans effet. Les fonctions de fermeture des ressources acceptent déjà 0.

## Tests

`examples/runtime/block_variables.oc` (auto-vérifiant, lancé par
`ci/regression.sh`) : objet, chaîne, tableau et fermeture de `init`/`main`
lus dans `success`/`exit`, `result SUCCESS` qui saute une déclaration ;
`--leak-check` sans fuite (aussi sur `examples/builtins/sqlite.oc`).

## Reste

L'attribution d'une fuite du `main` runtime à la ligne 0 dans le bilan
`--leak-check` (le `main` synthétisé n'a pas de position) n'est pas traitée.
