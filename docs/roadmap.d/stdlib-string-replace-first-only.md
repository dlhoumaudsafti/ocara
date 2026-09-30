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

## Correction

`String_replace` utilise désormais `str::replace` (toutes les occurrences),
comme documenté. Aucun exemple ne dépendait de l'ancien comportement —
`examples/builtins/string.oc` attendait même déjà `chien noir chien blanc`
(affichait `chien noir chat blanc`). Cas limite tranché : un `from` vide
laisse la chaîne inchangée (`str::replace("", to)` insérerait `to` entre
chaque caractère), documenté dans `docs/builtins/String.md`.

Tests : `runtime/src/tests/string.rs` (lancés par la nouvelle cible
`make tests-runtime` — `make tests` ne couvre que le crate `ocara`, les
tests du runtime ne tournaient jusqu'ici jamais via le Makefile), et
`examples/tests/67_named_argumentsTest.oc` (multi-occurrences).

## Priorité / Complexité

**Haute** (résultat silencieusement faux sur un builtin de base) —
**Simple** (une ligne côté runtime + test).

## Fichiers clés

`runtime/src/lib.rs` (`String_replace`), `docs/builtins/String.md`,
`examples/tests/` (nouveau test).
