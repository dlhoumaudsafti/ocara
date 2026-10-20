# `and` / `or` sans court-circuit ; condition `mixed` testée comme un pointeur — corrigé

Audit du 2026-10-19, corrigé le 2026-10-20.

## Constat

`and`/`or` évaluaient toujours les deux opérandes
(`n not equal null and n.v equal 1` levait « Cannot access 'v' on null »),
et une condition `mixed` testait le pointeur de la boîte (`if mixed(false)`
prenait la branche).

## Correctif (`src/lower/expr.d/lower.rs`)

- `lower_short_circuit` : `a and b` / `a or b` abaissés en blocs, `b`
  évalué seulement si nécessaire, ses temporaires rendus dans sa branche.
- `lower_condition` : une condition `mixed`/union est déballée
  (`__mixed_to_int` puis comparaison à 0) ; utilisée par `if`, `elseif`,
  `while`, `not`, `and`, `or`.
- EBNF §11 : court-circuit et valeur de vérité d'un `mixed` documentés.

## Tests

`examples/tests/99_short_circuit_indirect_callsTest.oc`.
