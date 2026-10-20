# Builtins sans type de retour déclaré : `int`/`bool` lus comme `mixed` — corrigé

Audit du 2026-10-19, corrigé le 2026-10-20.

## Constat

La table manuelle `fn_ret_types` (`src/lower/builder.d/program.rs`) n'avait
pas d'entrée pour plusieurs builtins : leur résultat était traité comme
`mixed` à l'affichage (`IO::writeln(File::size(f))` → SIGSEGV,
`${res.status()}`/`${Thread::currentId()}` → `null`, `${r.body()}` → une
chaîne perdue par appel).

## Correctif

- Les types de retour de tous les builtins sont déduits de leurs signatures
  (`src/builtins/`) pour les symboles absents de la table manuelle, qui
  reste prioritaire (`or_insert`).
- Les méthodes de `HTTPRequest` sont aussi enregistrées sous `HTTPResponse_*`
  (`builtin_call_owners`) : `res.ok()`, `res.body()` sont typés et leur
  résultat possédé est rendu (`call_ret_types`).

## Tests

`examples/tests/99_short_circuit_indirect_callsTest.oc`
(`builtinReturnTypesTest`).
