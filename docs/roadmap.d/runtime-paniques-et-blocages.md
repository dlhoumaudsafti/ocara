# Runtime : paniques Rust (abort) et blocages sur entrées limites

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

Une panique dans une fonction `extern "C"` ne peut pas se dérouler : le
processus s'arrête (« panic in a function that cannot unwind », rc 134).

| Appel | Résultat | Cause |
|---|---|---|
| `Math::clamp(5, 10, 0)` (`lo > hi`) | abort | `n.clamp(lo, hi)` (`runtime/src/lib.rs` ~l.985) |
| `.env` contenant `A="` | panique puis **blocage** (rc 124) | `value[1..len-1]` avec `len == 1` (`runtime/src/dotenv.rs` ~l.74) |
| `.env` contenant `=value` | abort | `std::env::set_var("")` (doc : `load()` ne lève jamais) |
| `System::setEnv` nom invalide | exception 103 correcte, mais trace de panique sur stderr | `set_var` non gardé |
| `System::sleep(-1)`, `Thread::sleep(-5)` | ne rend jamais la main | `ms as u64` (`lib.rs` ~l.1603, `thread.rs` ~l.189) |
| SQLite : `commit()` sans `prepare()` rattrapé, puis `rollback()` | **blocage définitif** | `match db.pending_query.lock().unwrap().clone() { None => throw… }` garde le verrou ; le `longjmp` saute son drop (`runtime/src/sqlite.rs` ~l.574) |

## Piste

- Valider les entrées et lever l'exception du builtin (MathException 103,
  durée négative → 0 ou exception).
- Règle générale : aucun `throw_*` (longjmp) pendant qu'un verrou ou une
  valeur Rust à destructeur est vivant — relâcher avant de lever. Rechercher
  les autres motifs « verrou temporaire + throw dans un match ».
- Envelopper le corps des `extern "C"` sensibles dans `catch_unwind`.

## Priorité / Complexité

**Haute** (crash, blocage) — **Légère**.

## Fichiers clés

`runtime/src/lib.rs`, `runtime/src/dotenv.rs`, `runtime/src/thread.rs`,
`runtime/src/sqlite.rs`, `runtime/src/system.rs`.
