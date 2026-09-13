# `scoped`/`consumed` jamais libérées au premier niveau d'un bloc runtime

## Constat

Découvert en vérifiant le correctif `Stmt::Result` de [memoire-double-free-et-fuites-scoped](memoire-double-free-et-fuites-scoped.md). Une variable `scoped`/`consumed` déclarée **directement** dans le corps d'un bloc `main`/`init`/`error`/`success`/`exit` (pas dans un bloc imbriqué comme un `if`) n'est **jamais** libérée, quelle que soit la façon dont le bloc se termine.

## Reproduction

```ocara
main {
    scoped s:array<int> = [1, 2, 3]
    IO::writeln("hi")
}
```
→ aucun appel `__array_free` généré (vérifié sur l'IR, `--dump --no-link`). Même résultat avec `result 0` à la place de la chute normale.

## Cause

Les blocs runtime sont assemblés et lowered par une fonction dédiée, `lower_runtime_main_manual` (`src/lower/builder.d/runtime.rs`), qui appelle `lower_stmt` directement sur chaque statement plutôt que de passer par `lower_block` (`src/lower/stmt.d/block.rs`) — elle ne pousse jamais de frame sur `block_scope_stack`. Conséquence en cascade :
- `register_owned_local` trouve `block_scope_stack` vide (rien à alimenter).
- `emit_scope_drops` n'est jamais invoquée pour ce niveau (elle est appelée exclusivement par `lower_block`, jamais par `lower_runtime_main_manual`).
- `emit_early_exit_drops` (utilisée par `return`/`result`/`break`/`continue`) n'a rien à parcourir : `block_scope_stack` reste vide.

La variable finit bien dans `owned_locals` (donc les mécanismes qui EN DÉPENDENT, comme `drop_consumed_used_in` pour un usage direct de `consumed`, fonctionnent normalement), mais rien ne la libère jamais si elle n'est pas explicitement "consommée" par un usage unique repéré dans le même statement.

## Portée

Touche potentiellement une grande partie des programmes Ocara réels : `main { }` est le bloc d'entrée quasi systématique. Toute `scoped`/`consumed` déclarée directement à son premier niveau (pas dans un `if`/`while`/etc. imbriqué, qui eux passent normalement par `lower_block`) fuit systématiquement.

## Pourquoi non corrigé dans le même chantier

`lower_runtime_main_manual` a une structure de contrôle particulière : deux phases de statements (le corps `init`+`main` concaténé, puis le bloc ERROR-check/exit/return) séparées par un saut vers `runtime_exit_bb`. Y ajouter correctement push/pop de `block_scope_stack` et l'équivalent d'`emit_scope_drops` demande le même niveau d'audit que ce qui a été fait pour `lower_block` (voir les 6 bugs corrigés dans le fichier ci-dessus) — risque de correctif partiel ou incorrect sans une session dédiée à cette seule fonction.

## Ampleur

Structurel : demande de faire fonctionner deux mécanismes conçus séparément (le lowering standard des blocs via `lower_block`, et l'assemblage spécial des blocs runtime) de façon cohérente, sans casser la structure `init`→`main`→(ERROR check)→`error`/`success`→`exit` existante.

## Fichiers clés

`src/lower/builder.d/runtime.rs` (`lower_runtime_main_manual`), `src/lower/stmt.d/block.rs` (mécanisme de référence à reproduire), `src/lower/stmt.d/ownership.rs`.
