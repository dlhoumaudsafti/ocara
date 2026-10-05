# Arguments de `use Classe(...)` ignorés pour une classe sans `init` — corrigé

## Constat

`use C("x")` sur `class C { public property name:string }` compilait : les
arguments étaient perdus et `c.name` valait `null`. Même chose pour une
classe qui étend un builtin sans déclarer d'`init` (`class MyErr extends
Exception {}` puis `use MyErr("boom", 2)`) : le constructeur du builtin ne
reçoit jamais les arguments.

## Correctif

Erreur **E60** (`SemaError::ArgsWithoutConstructor`, `Expr::New` dans
`src/sema/typecheck.rs`). Elle est émise quand la classe est déclarée dans le
programme, qu'aucun `init` n'existe dans sa chaîne d'ancêtres du programme,
et que des arguments sont passés. Restent acceptés :
- `use C()` ;
- une classe qui hérite d'un `init` utilisateur ;
- une classe dont l'`init` est généré (initialiseurs de `property`, `struct`) :
  son arité est vérifiée par le contrôle existant (`WrongArgCount`) ;
- un builtin construit directement (`use Exception("boom", 3)`).

L'option « constructeur par champs implicite » n'a pas été retenue : c'est
déjà le rôle de `struct`.

Aucune régression sur le corpus. Tests : `src/sema/tests/use_without_init.rs`.

## Bug trouvé

`parent::init(...)` sans effet pour un parent builtin d'exception → voir
[sema-builtin-parent-init-noop](sema-builtin-parent-init-noop.md).
