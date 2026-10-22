# Performances : mesures où PHP ou Perl battent Ocara

Relevé du 2026-10-22, premier passage de `make perf` comparé à six langages
(`ci/perf_compare.sh`, voir `ci/README.md`). La règle — aucune mesure plus
lente qu'un langage interprété — échoue sur 12 mesures de temps et une de
mémoire. Python n'est plus devant sur aucune ; PHP 8.3 l'est sur toutes
celles-ci, Perl sur deux.

## Temps (`make perf`)

| Mesure | Ocara | Plus rapide | Piste probable |
|---|---|---|---|
| `map` — `map<string, int>`, 10⁶ insertions + lectures | 0,27 s | PHP 0,12 s | clé construite par gabarit puis hachée par SipHash ; table de PHP sans graine aléatoire |
| `mapint` — `map<int, int>` | 0,11 s | PHP 0,03 s | clés stockées en texte (tableau « packed » côté PHP) |
| `mapiter` — parcours `for k has v in m` | 0,05 s | PHP 0,03 s | relecture de la clé texte à chaque tour |
| `split` — `String::split`, 6 morceaux | 0,12 s | PHP 0,09 s | une chaîne allouée par morceau |
| `replace` — `replace` + `upper` | 0,09 s | PHP 0,08 s | deux chaînes intermédiaires |
| `join` — `Array::join` + `intToStr` | 0,10 s | PHP 0,07 s | — |
| `jsonb` — `JSON::encode` + `decode` | 0,24 s | PHP 0,13 s | passage par `serde_json::Value` |
| `mixed` — arithmétique `mixed` / `int\|float` | 0,52 s | PHP 0,07 s, Perl 0,32 s | valeur boxée allouée à chaque opération (`__dyn_*`) |
| `file` — fichier de 2·10⁶ lignes | 0,28 s | PHP 0,21 s, Perl 0,26 s | découpage : une chaîne par ligne |
| `conv` — `strToInt` + `strToFloat` | 0,17 s | PHP 0,15 s | gabarit puis conversion |
| `sqlite` — insertions + `queryOne` | 0,15 s | PHP 0,12 s | map de placeholders construite à chaque appel |
| `date` — `DateTime::format` | 0,41 s | PHP 0,19 s | format analysé à chaque appel |

## Mémoire (`make perf-extra`)

| Mesure | Ocara | Plus économe |
|---|---|---|
| `mapint` | 69 Mio | PHP 49 Mio (tableau « packed » d'entiers) |

## Priorité / Complexité

**Moyenne** (règle de `make perf` non tenue) — **Légère** (format de date,
placeholders) à **Structurel** (clés entières natives, `mixed` sans boîte).

## Fichiers clés

`runtime/src/containers.rs`, `runtime/src/lib.rs` (`__dyn_*`, `String_*`,
JSON), `runtime/src/datetime.rs`, `runtime/src/sqlite.rs`,
`ci/perf_compare.sh`.
