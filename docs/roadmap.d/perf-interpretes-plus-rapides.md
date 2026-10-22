# Performances : égalités fragiles avec PHP

Relevé du 2026-10-22 de `make perf` comparé à six langages
(`ci/perf_compare.sh`, voir `ci/README.md`). Plus aucune mesure n'est
nettement plus lente qu'un langage interprété, en temps comme en mémoire
(`make perf-extra`) ; reste une série d'égalités avec PHP 8.3 où le bruit de
la machine fait parfois échouer la règle (`make perf` sorti en échec sur
`excls` et `file` au dernier passage, d'un centième).

## Égalités fragiles

| Mesure | Ocara | PHP | Piste probable |
|---|---|---|---|
| `excls` — exception de classe | 0,16–0,17 s | 0,16–0,17 s | — |
| `file` — fichier de 2·10⁶ lignes | 0,21–0,22 s | 0,21 s | découpage : une chaîne par ligne |
| `jsonb` — `JSON::encode` + `decode` | 0,13 s | 0,13 s | allocations par objet décodé (map, clés, tableau) ; un `float` de `mixed` est encore en cellule |
| `replace` — `replace` + `upper` | 0,08 s | 0,08 s | deux chaînes intermédiaires |
| `regex` — `Regex::test` + `replace` | 0,06 s | 0,06 s | — |
| `mapint` — `map<int, int>` | 0,03 s | 0,03–0,04 s | — |
| `conv` — `strToInt` + `strToFloat` | 0,14 s | 0,15–0,16 s | gabarit puis conversion |

## Priorité / Complexité

**Moyenne** (règle de `make perf` encore instable) — **Légère** à
**Moyenne** selon la mesure.

## Fichiers clés

`runtime/src/lib.rs` (`String_*`), `runtime/src/exception.rs`, `runtime/src/file.rs`, `runtime/src/json_codec.rs`, `ci/perf_compare.sh`, `ci/perf_report.py`.
