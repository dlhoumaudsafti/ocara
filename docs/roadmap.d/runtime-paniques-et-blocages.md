# Runtime : paniques Rust (abort) et blocages sur entrées limites — corrigé

Audit du 2026-10-19, corrigé le 2026-10-20.

## Constat et correctif

| Appel | Avant | Maintenant |
|---|---|---|
| `Math::clamp(5, 10, 0)` | abort (panique) | `MathException` 103 |
| `.env` `A="` / `=value` | blocage / abort | valeur `"` gardée / ligne ignorée |
| `.env` qui redéfinit `HOME` | variable système écrasée | jamais écrasée (DotEnv.md) |
| `System::setEnv("", …)` | exception + trace de panique | exception 103 seule (`valid_env_pair`) |
| `System::sleep(-1)`, `Thread::sleep(-5)` | jamais de retour | aucune attente |
| SQLite/MySQL `commit()` sans `prepare()` puis `rollback()` | blocage définitif | verrou relâché avant le `match` qui lève |

Règle appliquée : aucun `throw_*` (`longjmp`) pendant qu'un verrou est tenu.

## Tests

`examples/tests/98_stdlib_edge_valuesTest.oc` (`clamp`) ; reproductions
`DotEnv`, `sleep`, SQLite vérifiées à la main.
