# Blocs runtime : variables de `init`/`main` libérées avant `success`/`error`/`exit`

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

EBNF §5.3 : « les variables déclarées dans un bloc sont accessibles dans tous
les blocs suivants ». Or :

```ocara
class Box { public property v:int = 7 }
main {
    var b:Box = use Box()
    IO::writeln("main")
}
exit { IO::writeln(`${b.v}`) }      // affiche "main" puis SIGSEGV
```

- Objet : SIGSEGV dans `exit`, `success` ou `error` (après `result 1`).
- Chaîne / tableau : `null`. L'exemple de l'EBNF §5.7 (`init { var
  counter:int = 0 } main { counter = counter + 1 } success { … }`) affiche
  `Compteur: null`.
- `init` → `main` fonctionne.
- Inverse pour une ressource native : `const db:SQLite = SQLite::open(…)` dans
  `main`, fermée dans `exit`, n'est jamais libérée ; le bilan `--leak-check`
  l'attribue à la ligne 0 (`examples/builtins/sqlite.oc`).

## Cause

`lower_runtime_main_manual` (`src/lower/builder.d/runtime.rs`) regroupe
`init` + `main` dans un `Block` abaissé par `lower_block`, qui retire et
relâche les locales en sortie, avant `error`/`success`/`exit`.

## Priorité / Complexité

**Haute** (crash, fuite) — **Légère**.

## Fichiers clés

`src/lower/builder.d/runtime.rs`, `src/lower/stmt.d/block.rs`,
`src/core/leak_sites.rs` (ligne 0).
