# `async` / `resolve` : double libération, handles boxés, appels synchrones, tâches jamais libérées

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

1. **Deuxième `resolve` du même handle → double free (abort).**
   ```ocara
   async function sq(n:int): int { return n * n }
   var t:Resolvable<int> = sq(4)
   var a:int = resolve t
   var b:int = resolve t      // free(): double free detected
   ```
   EBNF §14.5 promet la valeur par défaut. `__task_resolve`
   (`runtime/src/lib.rs` ~l.3014) libère via `handle_take` sans marquer le
   handle consommé. Même chose avec deux boucles `for x in hs { resolve x }`.
2. **`Resolvable` ajouté par `push` puis résolu → SIGSEGV / `munmap_chunk`.**
   `hs.push(sq(3))` boxe le handle (`__box_int_for_mixed` avant
   `Array_push`) ; `resolve hs[0]` reçoit la boîte. Le motif courant
   (`for i … { ts.push(sq(i)) }` puis `for t in ts { tot += resolve t }`)
   plante dès une tâche. Le littéral `[h]`, la map et `hs[0] = sq(7)`
   fonctionnent.
3. **Appel `async` dans une `nameless` ou un générateur compilé en appel
   synchrone → SIGSEGV.** L'IR montre `Call sq` au lieu de `__task_spawn`,
   puis `resolve 9` déréférence l'entier. `nameless.rs` ne copie pas
   `async_funcs` ; `lower_message_func` (`message_gen.rs`) non plus.
4. **Tâche jamais résolue : le thread et son résultat ne sont jamais
   libérés** — environ 36 Ko par appel (733 Mo pour 20 000 appels).
   `--leak-check` : `ressource native` + `string` par appel. Le commentaire de
   `runtime/src/lib.rs` (~l.3003-3008) l'assume comme limite.
5. **Un thread système par appel `async`** (~30 µs/appel, 100 000 `clone3`
   pour 100 000 appels) — voir aussi
   [perf-runtime-complexite.md](perf-runtime-complexite.md).

## Piste

Handle à état (consommé → valeur par défaut, sans libération) ; ne jamais
boxer un `Resolvable` (type pointeur) ; propager `async_funcs` ; libérer le
handle non résolu à sa destruction (détacher le thread, rendre le résultat) ;
pool de threads.

## Priorité / Complexité

**Haute** (crash, fuite) — **Structurel**.

## Fichiers clés

`runtime/src/lib.rs` (`__task_spawn`, `__task_resolve`, `handle_take`),
`src/lower/expr.d/nameless.rs`, `src/lower/builder.d/message_gen.rs`,
`src/lower/expr.d/` (push d'un `Resolvable`).
