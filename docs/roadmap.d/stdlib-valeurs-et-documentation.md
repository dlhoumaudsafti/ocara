# Bibliothèque standard : valeurs fausses et documentation divergente

Audit du 2026-10-19 ; corrigé le 2026-10-20, sauf « Reste ».

## Corrigé

| Appel | Avant | Maintenant |
|---|---|---|
| `String::empty("   ")` | `false` | `true` |
| `String::split("", ",")` / `split("abc", "")` | `[""]` / 5 éléments | `[]` / `a,b,c` |
| `Math::round(-2.5)` | `-3` | `-2` (0,5 vers le haut, Math.md) |
| `Math::abs(INT_MIN)`, `Math::pow(2, 64)`, exposant > `u32` | valeurs fausses | `MathException` 105 (`OVERFLOW`) |
| dates avant 1970 (`DateTime`, `Date`, `Time`) | fausses (`1970-01-01T00:00:-1`) | justes (`runtime/src/civil.rs`, algorithmes de H. Hinnant) |
| `2023-02-31` | basculait au mois suivant | `DateTimeException`/`DateException` 101 |
| `File::infos` / `Directory::infos` | `is_file` = `1`, `created` en chaîne, `size` brut | valeurs boxées : `bool`, `int` |
| `File::copy(p, p)` | fichier vidé | sans effet |
| `File::writeBytes` hors 0..255 | tronqué | `FileException` 104 |
| `Array::slice` avec négatifs | incohérent | bornes ramenées dans `[0, len]` |
| SQLite BLOB | `null` | `array<int>` d'octets ; placeholder non bindable → 106 |
| `IO::read` en fin d'entrée | `""` | `IOException` 101 |
| attribut `HTMLComponent` entier `0` | `null` | `0` (valeurs boxées) |
| constante `7.0 % 2.0` | refusée | acceptée |
| `1.0e300` | « undefined symbol 'e300' » | « exponent notation is not supported » |
| gabarit `${String::len("{x")}` | « unterminated string » | accepté |

Documentation alignée : Convert.md, DateTime.md, Time.md, DotEnv.md (copie
en double retirée), JSON.md, YAML.md, UnitTest.md (`assertEnum`), Regex.md
(`$1`, `${nom}`, `$$`), Math.md (code 105), String.md, File.md, SQLite.md,
Array.md, EBNF §11.1.

Tests : `examples/tests/98_stdlib_edge_valuesTest.oc`.

## Reste

- `INT_MIN` non écrivable en littéral (`-9223372036854775808` → « integer too
  large ») : le lexer ne sait pas que le `-` précède le littéral ; à traiter
  au parseur (négation d'un littéral hors borne).
- Un `float` entier s'affiche sans décimale (`42.0` → `42`), indiscernable
  d'un `int` : choix de format à trancher (modifie de nombreuses sorties).

## Priorité / Complexité

**Basse** (reste) — **Simple**.
