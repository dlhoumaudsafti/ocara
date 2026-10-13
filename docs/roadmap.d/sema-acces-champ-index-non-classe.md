# Accès de champ et indexation sur `int`/`float`/`bool`/`void` acceptés — SIGSEGV

Statut : **corrigé** le 2026-10-13 (constaté le même jour par l'audit de la roadmap).

## Constat

E36 et E37 refusent un **appel de méthode** sur un résultat `void` ou un
primitif (voir [langage-appel-methode-sur-void-accepte](langage-appel-methode-sur-void-accepte.md),
[langage-appel-methode-sur-primitif-accepte](langage-appel-methode-sur-primitif-accepte.md)),
mais pas un **accès de champ** ni une **indexation** :

```ocara
var x:int = 42
IO::writeln(x.nonexistent)      // compile, puis SIGSEGV (exit 139)
IO::writeln(x[3])               // compile, puis SIGSEGV
// idem b.foo (bool), fl.foo (float), f.getCount().foo
IO::writeln(f.doIt().x)         // doIt(): void — SIGSEGV
IO::writeln(f.doIt()[0])        // doIt(): void — compile, affiche « null »
```

Pour `string` et `array`, l'accès est correctement refusé (« field not found
in class 'String' »).

## Cause

`src/sema/typecheck.rs` :
- `Expr::Field` hors appel (~l.1163-1166) : quand `type_class_name` rend
  `None`, le type devient `Type::Mixed` sans erreur ;
- `Expr::Index` (~l.2173-2177) : le cas `_ => Type::Mixed` laisse passer tout
  type.

## Correction attendue

Même mécanisme qu'E36/E37 : refuser (E36 pour `void`, E37 pour un primitif)
l'accès de champ et l'indexation sur `int`/`float`/`bool`/`null`/`void`/
`Function`/`message<T>`/`Resolvable<T>`. `mixed` reste permissif (voir
[langage-exception-chaine-champ-message](langage-exception-chaine-champ-message.md)
pour son cas).

## Validation

Tests sema (`src/sema/tests/method_call_on_non_class.rs`,
`method_call_on_void.rs`) étendus aux champs et index ; `make regression`.

## Priorité / Complexité

**Haute** (programme invalide accepté qui plante) — **Simple**.

## Correctif

Nouveau diagnostic **E63** (`SemaError::AccessOnNonContainer`) : champ lu ou indexation sur `int`/`float`/`bool`/`null`/`void`/`Function`/`message<T>`/`Resolvable<T>` (`has_no_members`, `src/sema/typecheck.rs`). `mixed` reste permissif. Tests : `src/sema/tests/access_on_non_container.rs`.
