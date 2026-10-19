# Bibliothèque standard : valeurs fausses et documentation divergente

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Valeurs fausses

| Appel | Obtenu | Attendu (doc) |
|---|---|---|
| `String::empty("   ")` | `false` | `true` (String.md ~l.158) |
| `String::split("", ",")` | `[""]` | vide (String.md ~l.210) ; `split("abc", "")` rend 5 éléments |
| `Math::round(-2.5)` | `-3` | `-2` (Math.md ~l.167) |
| `Math::abs(INT_MIN)` | `INT_MIN` | exception ou valeur définie |
| `Math::pow(2, 64)`, `pow(10, 19)`, `pow(2, 4294967296)` | `0`, négatif, `1` (exposant tronqué en `u32`) | dépassement signalé |
| `DateTime::fromTimestamp(-1)` | `1970-01-01T00:00:-1` | `1969-12-31T23:59:59` |
| `DateTime::parse("1969-12-31T23:59:59")`, `parse("1900-01-01 00:00:00")` | `31535999`, `0` | `-1`, négatif |
| `Date::addDays("1970-01-05", -10)` / `diffDays("1969-12-31","1970-01-01")` / `dayOfWeek("1969-12-31")` | `1970-01--5` / `364` / `3` | `1969-12-26` / `1` / `2` |
| `Time::fromSeconds(-1)` | `00:00:-1` | `23:59:59` ou exception |
| `DateTime::parse("2023-02-31T00:00:00")` | `2023-03-03` | exception 101 (DateTime.md) |
| `File::infos` / `Directory::infos` | `is_file` = `1`, `is_dir` = `null`, `created`/`modified` en chaîne (vu via `mixed`/JSON) | bool et int (File.md ~l.115) |
| `File::copy(p, p)` | **fichier vidé** (perte de données) | sans effet ou exception |
| SQLite colonne BLOB (`x'00ff10'`) | `null` | octets ; tableau en placeholder → code 102 au lieu de 106 |
| `File::writeBytes` avec 256 / -1 | 0 / 255 silencieusement | exception |
| `Array::slice(a, -2, 5)` / `slice(a, 1, -1)` | vide / borné à la fin | comportement cohérent des négatifs |
| `IO::read` en fin d'entrée | `""` | `IOException` (IO.md ~l.203) |
| `DotEnv::load` | écrase les variables système (`HOME`) | ne les écrase pas (DotEnv.md ~l.220) |
| Regex : remplacement `"$x"` | `""` (expansion `$1`/`$x` non documentée) | documenter ou échapper |
| `HTMLComponent` : attribut int `0` lu en string | `null` | `"0"` |

Causes des dates : boucles d'années à partir de 1970 et `%`/`/` tronqués
(`runtime/src/datetime.rs` `timestamp_to_parts`/`parts_to_timestamp`,
`runtime/src/date.rs`) ; jour validé seulement contre 1..31.

## Documentation divergente

- Convert.md (~l.56-73) : `strToInt`/`strToFloat` rendraient 0 sur entrée
  invalide ; ils lèvent 101/102 (comme le dit Convert.md ~l.307).
- DateTime.md (~l.114) : `parse` rendrait 0 ; il lève 101. L'exemple
  `fromTimestamp(1714233600)` → « 18:00:00 » vaut `16:00:00` UTC (Time.md
  idem).
- DotEnv.md contient tout son contenu deux fois (l.1-226 et 228-445).
- JSON.md : « floats pas encore supportés » (ils le sont), « nombres décodés
  en `int` », exemples à clés en ordre d'insertion alors que la sortie est
  triée.
- YAML.md : `!!foo bar` décoderait en `null` ; il rend `"bar"`.
- UnitTest.md (~l.230) : `assertEnum` échouerait (117) ; il passe toujours.
- EBNF.md (~l.1493) : `validate(42.0)` comparé à `42` vaudrait `false` ; il
  vaut `true`.

## Petites limites du langage

- `INT_MIN` non écrivable en littéral (`-9223372036854775808` → « integer too
  large ») alors que `Convert::strToInt` l'accepte.
- `public const E:float = 7.0 % 2.0` refusé : `fold_float` n'a pas de cas
  `Mod` (`src/parsing/ast.d/const_fold.rs` ~l.52).
- `1.0e300` → « undefined symbol 'e300' » : pas de littéral à exposant
  (conforme à l'EBNF), message à améliorer.
- Gabarit : une chaîne contenant `{` ou `}` dans `${…}` casse le lexer
  (`` `${String::len("{x")}` `` → « unterminated string »).
- Float entier affiché sans décimale (`42.0` → `42`), indiscernable d'un int.

## Priorité / Complexité

**Haute** (valeurs fausses, perte de données `File::copy`) — **Légère**.

## Fichiers clés

`runtime/src/lib.rs`, `runtime/src/datetime.rs`, `runtime/src/date.rs`,
`runtime/src/file.rs`, `runtime/src/sqlite.rs`, `runtime/src/dotenv.rs`,
`docs/builtins/*.md`, `docs/EBNF.md`.
