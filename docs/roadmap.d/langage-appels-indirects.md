# Appels indirects et générateurs statiques — corrigé

Audit du 2026-10-19, corrigé le 2026-10-20.

## Constat

`adder(4)(1)`, `fs[0](5)`, `m["d"](21)` → « fonction inconnue : _unknown » ;
variable d'un `for` sur `message<Function<…>>` non appelable ; une méthode
générateur statique cassait la compilation même jamais appelée ; un champ
`Function` n'était pas appelable (`h.f()`).

## Correctif

- `callee_function_ret` (`src/lower/expr.d/helpers.rs`) : type de retour
  d'une callee index, appel ou champ `Function` ; `call_fat_ptr` factorise
  l'appel indirect ; résultat compté rendu comme temporaire. Sema :
  `check_expression_call` (types d'arguments vérifiés, variable marquée
  utilisée) ; un champ `Function` appelé n'est plus une erreur.
- `register_loop_var_meta` partagé par la boucle sur tableau et la boucle sur
  générateur.
- Les méthodes générateurs statiques n'ont plus de wrapper `__fn_wrap_`.

## Tests

`examples/tests/99_short_circuit_indirect_callsTest.oc`.
