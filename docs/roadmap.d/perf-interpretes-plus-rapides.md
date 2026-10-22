# Performances : mesures où PHP ou Perl battent Ocara

Relevé du 2026-10-22 de `make perf` comparé à six langages
(`ci/perf_compare.sh`, voir `ci/README.md`). La règle — aucune mesure plus
lente qu'un langage interprété — échoue sur 7 mesures de temps ; aucune en
mémoire (`make perf-extra`). Python n'est plus devant sur aucune ; PHP 8.3
l'est sur toutes celles-ci, Perl sur deux.

## Temps (`make perf`)

| Mesure | Ocara | Plus rapide | Piste probable |
|---|---|---|---|
| `split` — `String::split`, 6 morceaux | 0,12 s | PHP 0,09 s | une chaîne allouée par morceau |
| `join` — `Array::join` + `intToStr` | 0,10 s | PHP 0,07 s | — |
| `jsonb` — `JSON::encode` + `decode` | 0,22 s | PHP 0,13 s | passage par `serde_json::Value` |
| `mixed` — arithmétique `mixed` / `int\|float` | 0,48 s | PHP 0,07 s, Perl 0,33 s | valeur boxée allouée à chaque opération (`__dyn_*`) |
| `file` — fichier de 2·10⁶ lignes | 0,27 s | PHP 0,21 s, Perl 0,26 s | découpage : une chaîne par ligne |
| `sqlite` — insertions + `queryOne` | 0,14 s | PHP 0,12 s | map de placeholders construite à chaque appel |
| `date` — `DateTime::format` | 0,41 s | PHP 0,19 s | format analysé à chaque appel |

## Égalités fragiles

À égalité avec PHP ou devant d'un centième : la règle passe, mais le bruit
de la machine la fait parfois échouer.

| Mesure | Ocara | PHP | Piste probable |
|---|---|---|---|
| `replace` — `replace` + `upper` | 0,08 s | 0,08 s | deux chaînes intermédiaires |
| `arrops` — `contains`/`indexOf`/`slice`/`reverse`/`pop` | 0,22 s | 0,22 s | — |
| `excls` — exception de classe | 0,17 s | 0,17 s | — |
| `regex` — `Regex::test` + `replace` | 0,06 s | 0,06 s | — |
| `map` — `map<string, int>` | 0,11 s | 0,12 s | clé gabarit formatée à chaque accès |
| `conv` — `strToInt` + `strToFloat` | 0,14 s | 0,15 s | gabarit puis conversion |

## Priorité / Complexité

**Moyenne** (règle de `make perf` non tenue) — **Légère** (format de date,
placeholders) à **Structurel** (`mixed` sans boîte).

## Fichiers clés

`runtime/src/lib.rs` (`__dyn_*`, `String_*`, JSON), `runtime/src/datetime.rs`,
`runtime/src/sqlite.rs`, `ci/perf_compare.sh`, `ci/perf_report.py`.
