# Appel de méthode inexistante sur une variable d'exception — corrigé

## Constat

Dans un handler `on e is Exception`, `e.message()` (au lieu du champ
`e.message`) et `e.nothing()` compilaient sans diagnostic et valaient `null`
à l'exécution. Cause : le binding du handler était déclaré `mixed`
(`Stmt::Try` dans `src/sema/typecheck.rs`), et un appel de méthode sur un
`mixed` est permissif.

## Correctif

- `on e is X` : `e` est typé `Type::Named(X)` quand `X` est une classe connue
  (le filtre le garantit). Le handler générique `on e` garde `mixed`.
- Nouveau diagnostic **E57** (`SemaError::FieldCalledAsMethod`) quand le nom
  appelé est un champ de la classe ou d'un ancêtre (`lookup_field_owner`) :
  « 'message' is a field of 'Exception', not a method — write '.message'
  without parentheses ». Une méthode réellement inexistante reste signalée
  par `FieldNotFound`. E57 s'applique aussi aux classes utilisateur.
- Documentation : `docs/diagnostics.md` (E57), `docs/EBNF.md` §29.2 (type du
  binding).

Aucune régression sur le corpus (952 + 50 tests ocaraunit, 75 exemples).

## Fichiers clés

`src/sema/typecheck.rs`, `src/sema/error.rs`,
`src/sema/tests/exception_binding.rs`.
