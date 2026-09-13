# `scoped`/`consumed` jamais libérées au premier niveau d'un bloc runtime

## ✅ Corrigé

## Constat (avant correctif)

Découvert en vérifiant le correctif `Stmt::Result` de [memoire-double-free-et-fuites-scoped](memoire-double-free-et-fuites-scoped.md). Une variable `scoped`/`consumed` déclarée **directement** dans le corps d'un bloc `main`/`init`/`error`/`success`/`exit` (pas dans un bloc imbriqué comme un `if`) n'était **jamais** libérée, quelle que soit la façon dont le bloc se terminait.

## Reproduction (avant correctif)

```ocara
main {
    scoped s:array<int> = [1, 2, 3]
    IO::writeln("hi")
}
```
→ aucun appel `__array_free`/`__value_free` généré (vérifié sur l'IR, `--dump --no-link`). Même résultat avec `result 1` à la place de la chute normale, et pour un `exit { scoped ... }`.

## Cause

Les blocs runtime sont assemblés et lowered par une fonction dédiée, `lower_runtime_main_manual` (`src/lower/builder.d/runtime.rs`). Elle appelait `lower_stmt` directement sur chaque statement de `init`+`main` et de `exit` plutôt que de passer par `lower_block` (`src/lower/stmt.d/block.rs`) — elle ne poussait jamais de frame sur `block_scope_stack` pour ces deux portions. Conséquence en cascade :
- `register_owned_local` trouvait `block_scope_stack` vide (rien à alimenter).
- `emit_scope_drops` n'était jamais invoquée pour ce niveau (elle n'est appelée que par `lower_block`, jamais par `lower_runtime_main_manual`).
- `emit_early_exit_drops` (utilisée par `result`) n'avait rien à parcourir : `block_scope_stack` restait vide.

**Important** : `error`/`success` n'étaient PAS concernés par ce bug — `generate_runtime_main` les enveloppe déjà dans un vrai `Block`, passé en `then_block`/`else_block` d'un `Stmt::If` unique (`if ERROR != 0 { error } else { SUCCESS = true; success }`), lowered via `lower_if` → `lower_block` comme n'importe quel `if` normal. Seuls `init`+`main` (concaténés à plat) et `exit` (ajouté à plat en fin de liste) contournaient `lower_block`.

## Correctif

`lower_runtime_main_manual` enveloppe maintenant explicitement les statements de `init`+`main` d'une part, et de `exit` d'autre part, dans un vrai `Block` AST, et les lower via `lower_block` (au lieu d'une boucle `lower_stmt` brute) — exactement le même mécanisme que pour un `if`/`while`/corps de fonction normal.

Seules `ERROR`/`SUCCESS` (les 2 premières déclarations, injectées par `generate_runtime_main`) restent lowered directement hors de tout `Block` : ce sont des `var` (jamais suivies par `owned_locals`) qui doivent rester lisibles/modifiables dans toutes les phases suivantes (le check `if ERROR != 0`, `exit`, le `return ERROR` final) — les passer par `lower_block` les aurait fait disparaître de `builder.locals` en sortie de bloc (mécanisme de restauration anti-shadowing, voir [memoire-double-free-et-fuites-scoped](memoire-double-free-et-fuites-scoped.md)).

Effet de bord correct et voulu : une variable déclarée dans `main{}` n'est désormais plus visible dans `exit{}` (chaque bloc source a vraiment sa propre portée, cohérent avec le reste du langage) — auparavant elle restait accessible indéfiniment dans `builder.locals`, un flou jamais exploité par aucun exemple du dépôt.

Vérifié :
- IR généré (`--dump --no-link`) : `__value_free`/`__array_free` bien émis en fin de `main{}` et de `exit{}` pour une `scoped`/`consumed` déclarée directement dedans.
- Un `result` anticipé à l'intérieur de `main{}` (dans un `if` imbriqué) libère correctement sur son propre chemin, sans double-free sur le chemin de chute normale (chemins mutuellement exclusifs, chacun avec son propre `__value_free`).
- `make regression` : aucune régression (386 PASS / 0 FAIL sur les tests unitaires, tous les exemples `01`–`32` + `project/main` + tous les builtins OK, seule l'erreur pré-existante et déjà documentée `mainTest.oc`/`Printable` subsiste).

## Fichiers clés

`src/lower/builder.d/runtime.rs` (`lower_runtime_main_manual`, `generate_runtime_main`), `src/lower/stmt.d/block.rs` (mécanisme réutilisé tel quel), `src/lower/stmt.d/ownership.rs`.
