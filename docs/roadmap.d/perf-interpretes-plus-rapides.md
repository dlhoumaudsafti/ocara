# Performances : mesures où PHP ou Perl battent Ocara

Relevé du 2026-10-22 de `make perf` comparé à six langages
(`ci/perf_compare.sh`, voir `ci/README.md`). La règle — aucune mesure plus
lente qu'un langage interprété — échoue sur 5 mesures de temps ; aucune en
mémoire (`make perf-extra`). Python et Perl ne sont plus devant sur
aucune ; PHP 8.3 l'est sur toutes celles-ci.

## Temps (`make perf`)

| Mesure | Ocara | Plus rapide | Piste probable |
|---|---|---|---|
| `split` — `String::split`, 6 morceaux | 0,12 s | PHP 0,09 s | une chaîne allouée par morceau |
| `join` — `Array::join` + `intToStr` | 0,09 s | PHP 0,07 s | — |
| `jsonb` — `JSON::encode` + `decode` | 0,21 s | PHP 0,13 s | passage par `serde_json::Value` |
| `file` — fichier de 2·10⁶ lignes | 0,22 s | PHP 0,21 s | découpage : une chaîne par ligne |
| `sqlite` — insertions + `queryOne` | 0,13 s | PHP 0,11 s | map de placeholders construite à chaque appel |

## Égalités fragiles

À égalité avec PHP ou devant d'un centième : la règle passe, mais le bruit
de la machine la fait parfois échouer (`excls` : PHP oscille entre 0,16 et
0,17 s).

| Mesure | Ocara | PHP | Piste probable |
|---|---|---|---|
| `excls` — exception de classe | 0,17 s | 0,16–0,17 s | — |
| `mixed` — arithmétique `mixed` / `int\|float` | 0,07 s | 0,07 s | deux appels `__dyn_*` par tour ; un `float` de `mixed` est encore en cellule |
| `replace` — `replace` + `upper` | 0,08 s | 0,08 s | deux chaînes intermédiaires |
| `regex` — `Regex::test` + `replace` | 0,06 s | 0,06 s | — |
| `mapint` — `map<int, int>` | 0,03 s | 0,03–0,04 s | — |
| `conv` — `strToInt` + `strToFloat` | 0,14 s | 0,15–0,16 s | gabarit puis conversion |

## Priorité / Complexité

**Moyenne** (règle de `make perf` non tenue) — **Légère** (placeholders) à
**Moyenne** (découpage sans chaîne par morceau).

## Fichiers clés

`runtime/src/lib.rs` (`String_*`, JSON), `runtime/src/sqlite.rs`, `ci/perf_compare.sh`, `ci/perf_report.py`.
