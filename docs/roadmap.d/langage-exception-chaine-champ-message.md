# `e.message` après un `raise "chaîne"` dans un handler générique — SIGSEGV

Statut : **corrigé** le 2026-10-13 (constaté le même jour par l'audit de la roadmap).

## Constat

```ocara
try {
    raise "direct"
} on e {
    IO::writeln(e.message)     // exit 139 (SIGSEGV)
}
```

Même résultat dans une fonction, une closure ou une méthode. Plus largement,
un accès de champ sur un `mixed` qui contient une chaîne lit n'importe quoi :
`var x:mixed = "abc"` puis `x.message` affiche un flottant aberrant, sans
erreur.

C'est très probablement le « bug distinct… setjmp/longjmp, en cours
d'investigation » cité en commentaire dans
`examples/advanced/tauri_httpserver/main.oc` (~l.54).

## Cause

Dans `on e` (sans filtre), `e` est `mixed` ; la sema accepte tout accès de
champ sur `mixed`, et le lowering émet `GetField message offset 0` sur le
pointeur de chaîne (visible avec `--dump`).

## Correction attendue

Au choix (à trancher) :
- garde à l'exécution : un `GetField` sur un `mixed` vérifie que la valeur est
  un objet de la bonne classe (sinon `null` ou exception) ;
- ou, dans un handler générique, refuser `e.<champ>` en sema et exiger
  `on e is Exception` (le message d'erreur l'indique).

`${e}` d'une chaîne levée est déjà correct (voir
[langage-raised-string-interpolation](langage-raised-string-interpolation.md)).

## Validation

Test `examples/tests/` : `raise "x"` puis `e.message` (comportement choisi),
`raise use Exception("x")` puis `e.message` ; corriger le commentaire de
`tauri_httpserver/main.oc`.

## Priorité / Complexité

**Haute** (crash sur un motif courant de gestion d'erreur) — **Légère**.

## Correctif

Choix retenu : garde à l'exécution. Dans un handler générique `on e`, un accès de champ sur `e` passe par `__exc_get_field` (`runtime/src/exception.rs`) : pour une chaîne levée, `e.message` rend la chaîne elle-même et tout autre champ vaut `null` ; pour une exception, lecture normale. Bindings suivis par `LowerBuilder::generic_catch_vars` (`src/lower/stmt.d/statements.d/exceptions.rs`, `message_gen.rs` pour un générateur). Test : `examples/tests/89_generators_resolve_and_catchTest.oc` (`messageOfRaisedStringTest`). Le cas plus général d'un champ lu sur un `mixed` non objet hors handler reste permissif (limitation du type `mixed`, voir W02).
