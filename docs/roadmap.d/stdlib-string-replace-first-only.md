# `String::replace` ne remplace que la première occurrence (contredit sa documentation)

## Constat (reproduit)

```ocara
IO::writeln(String::replace("a_b_c", "_", "-"))   // affiche "a-b_c"
```

`docs/builtins/String.md` promet explicitement le contraire : « Remplace
**toutes** les occurrences », avec l'exemple `String::replace("a-b-c", "-", ".")
// → "a.b.c"`, et un avertissement dédié opposant `String::replace` (toutes
les occurrences) à `Regex::replace` (première seulement).

Cause : `runtime/src/lib.rs`, `String_replace` appelle
`src.replacen(from_s, to_s, 1)` au lieu de `src.replace(from_s, to_s)`.

Découvert en écrivant l'exemple 67 (arguments nommés) — sans rapport avec
eux, reproduit en appel purement positionnel.

## À trancher

Corriger le runtime (la documentation décrit clairement l'intention, et
l'avertissement `Regex::replace` n'a de sens que dans ce sens) — vérifier
d'abord qu'aucun exemple ne dépend du comportement actuel (première
occurrence seulement), et ajouter un test ocaraunit multi-occurrences.

## Priorité / Complexité

**Haute** (résultat silencieusement faux sur un builtin de base) —
**Simple** (une ligne côté runtime + test).

## Fichiers clés

`runtime/src/lib.rs` (`String_replace`), `docs/builtins/String.md`,
`examples/tests/` (nouveau test).
