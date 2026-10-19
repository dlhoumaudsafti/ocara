# Appels indirects et générateurs statiques : erreurs de codegen sur du code valide

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

1. **Appeler le résultat d'un appel ou d'un index** : `adder(4)(1)`,
   `fs[0](5)`, `m["d"](21)` → `codegen error: appel vers une fonction
   inconnue : _unknown`, plus un faux avertissement « variable 'fs' is never
   used ». Une callee qui n'est ni `Ident` ni `Field` tombe dans
   `_ => "_unknown"` (`src/lower/expr.d/lower.rs` ~l.810).
2. **Variable d'un `for` sur un `message<Function<…>>` non appelable** :
   `for f in gen("x") { f() }` → `appel vers une fonction inconnue : f` (le
   chemin générateur ne remplit pas `func_vars`, contrairement à
   `loops.rs` pour un tableau).
3. **Déclarer une méthode générateur statique casse la compilation**, même
   jamais appelée : `class Bag { public static method one(): message<int> {
   emit 7 } }` → `appel vers une fonction inconnue : Bag_one`.
4. **Champ `Function` non appelable directement** : `h.f()` → « 'f' is a
   field of 'Holder', not a method » (il faut `var g = h.f; g()`).

## Piste

Abaisser toute callee expression en valeur fat-pointer puis appel indirect
(comme pour une variable `Function`).

## Priorité / Complexité

**Haute** (code valide rejeté) — **Légère**.

## Fichiers clés

`src/lower/expr.d/lower.rs`, `src/lower/builder.d/message_gen.rs`,
`src/lower/builder.d/classes.rs`, `src/sema/typecheck.rs` (champ appelé).
