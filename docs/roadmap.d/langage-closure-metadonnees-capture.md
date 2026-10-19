# Variable capturée par une fermeture : métadonnées perdues (map indexée → SIGSEGV, fuite)

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

Dans le corps d'une `nameless`, une variable capturée garde seulement sa
classe (`var_class`). Ses autres métadonnées (`map_vars`, `elem_ast_types`,
`func_ret_types`, `func_ret_ast`) ne sont pas propagées :

1. **Map capturée lue ou écrite par index → SIGSEGV ou valeur fausse.**
   ```ocara
   var m:map<string,int> = {}
   var f:Function<void()> = nameless(): void { m["a"] = 5 }
   f()
   IO::writeln(m["a"])      // SIGSEGV
   ```
   Lecture `m["a"]` dans la fermeture → 0 ; `map<int,int>` → 0 ;
   `m["a"] += 1`, `m["a"]++`, `arr[0]["k"] = v` plantent. `is_map_target`
   répond faux, d'où `__array_set`/`__array_get` sur une map. Casse le partage
   d'une map entre threads, handlers HTTP et `withLock` (`Map::set` fonctionne).
2. **Fonction capturée qui rend une `string`, appelée dans la fermeture :
   fuite d'une chaîne par appel** (320 000 chaînes pour 16 threads × 20 000).
   ```ocara
   var fn:Function<string(int)> = nameless(x:int): string { return `v-${x}` }
   var g:Function<void()> = nameless(): void { var r:string = fn(1) }
   ```

## Piste

Propager toutes les tables de métadonnées par nom capturé dans
`src/lower/expr.d/nameless.rs` (autour des l.183-195), comme pour `var_class`.

## Priorité / Complexité

**Haute** (crash, fuite) — **Simple**.

## Fichiers clés

`src/lower/expr.d/nameless.rs`, `src/lower/expr.d/helpers.rs` (`is_map_target`).
