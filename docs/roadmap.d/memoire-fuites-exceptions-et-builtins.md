# Fuites mémoire : exceptions, méthodes d'instance JSON, HTMLComponent — corrigé

Audit du 2026-10-19, corrigé le 2026-10-20 (`--leak-check` sans fuite sur
chaque reproduction, `make leak-check` sans fuite nouvelle).

## Correctifs

- **`raise` dans un handler `on`** : l'exception rattrapée et son type sont
  inscrits sur la pile de déroulement pendant l'appel du handler
  (`hold_raised`/`release_held`, `runtime/src/lib.rs`) ; un `raise` qui
  saute la frame les rend.
- **`return` dans un handler `on`** : la fonction englobante rend ses
  locales avant de retourner ; dans le `main` runtime, seules les portées
  internes (`release_before_runtime_exit`).
- **Constructeur qui lève** : l'objet neuf est inscrit sur la pile de
  déroulement pendant la construction (`guard_new_object`,
  `src/lower/expr.d/lower.rs`).
- **Cellule de capture** (variable de boucle au nom réutilisé + `raise`) : les
  variables d'une boucle `for` ne survivent plus à la boucle
  (`save_loop_vars`/`restore_loop_vars`, `src/lower/stmt.d/statements.rs`).
- **`m.encode()`, `s.decode()`, `s.pretty()`…** : résultat enregistré comme
  temporaire possédé.
- **`HTMLComponent`** : le registre garde la closure retenue et la rend quand
  elle est remplacée, désenregistrée ou en fin de programme
  (`release_registry`) ; l'instance native est partagée par nom
  (`interned`), plus une allocation perdue par `use HTMLComponent(…)`.

## Outil

`grep -n "Box::into_raw\|Box::from_raw" runtime/src/*.rs` liste les builtins
dont l'instance Rust n'a pas de destruction (fuites invisibles à
`--leak-check`).
