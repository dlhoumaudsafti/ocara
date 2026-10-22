# Performances : égalités fragiles avec PHP

Relevé du 2026-10-22 de `make perf` comparé à six langages
(`ci/perf_compare.sh`, voir `ci/README.md`) : la règle — aucune mesure plus
lente qu'un langage interprété — passe, en temps comme en mémoire
(`make perf-extra`). Reste une série de mesures à égalité avec PHP 8.3 ou
devant d'un seul centième, où le bruit de la machine peut la faire échouer.

## Égalités fragiles

| Mesure | Ocara | Meilleur interprété | Piste probable |
|---|---|---|---|
| `replace` — `replace` + `upper` | 0,08 s | PHP 0,08 s | deux chaînes intermédiaires |
| `join` — `Array::join` + `intToStr` | 0,06–0,07 s | PHP 0,07 s | une chaîne par `intToStr` |
| `jsonb` — `JSON::encode` + `decode` | 0,13 s | PHP 0,13 s | allocations par objet décodé (map, clés, tableau) ; un `float` de `mixed` est encore en cellule |
| `regex` — `Regex::test` + `replace` | 0,06 s | PHP 0,06 s | — |
| `map` — `map<string, int>` | 0,10–0,11 s | PHP 0,12 s | — |
| `split` — `String::split` | 0,08 s | PHP 0,09 s | une chaîne allouée par morceau |
| `mapint` — `map<int, int>` | 0,03 s | PHP 0,03–0,04 s | — |
| `mixed` — arithmétique `mixed` | 0,06 s | PHP 0,07 s | deux appels `__dyn_*` par tour ; un `float` de `mixed` est encore en cellule |

## Priorité / Complexité

**Basse** à **Moyenne** (la règle passe, mais sans marge) — **Légère** à
**Moyenne** selon la mesure.

## Fichiers clés

`runtime/src/lib.rs` (`String_*`, `__dyn_*`), `runtime/src/json_codec.rs`, `runtime/src/regex.rs`, `ci/perf_compare.sh`, `ci/perf_report.py`.
