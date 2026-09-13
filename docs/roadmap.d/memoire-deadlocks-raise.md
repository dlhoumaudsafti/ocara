# Blocages (deadlocks) provoqués par `raise` traversant un verrou tenu

## Mécanisme en cause

`raise` est implémenté via `setjmp`/`longjmp` (runtime/src/lib.rs:2385, 2451, 2500), qui saute par-dessus les frames Rust sans exécuter leurs destructeurs (`Drop`).

## Cas concrets

- **SQLite/MySQL** : `SQLite_execute`/`SQLite_query` (et équivalents MySQL) prennent un verrou (`db.conn.lock().unwrap()`) puis, en cas d'erreur SQL, appellent `throw_sqlite_exception` → `longjmp`. Le `MutexGuard` Rust ne voit jamais son `Drop` s'exécuter → le mutex reste verrouillé indéfiniment → toute requête suivante sur la même connexion se bloque. Reproductible avec un simple `try { db.execute("SQL invalide") } on {...}`. (runtime/src/sqlite.rs:70-95, 105-129, 179-203)
- **`Mutex` Ocara** : l'API `lock()`/`unlock()` est manuelle (pas de RAII côté langage, src/builtins/mutex.rs:41-56). Un `raise` entre les deux saute l'`unlock()` → mutex jamais déverrouillé → tous les threads en attente bloquent indéfiniment.

## Ampleur

- Fix ponctuel pour SQLite/MySQL : relâcher explicitement le verrou (`drop(conn)`) avant chaque `throw_*` — petit correctif, mais seulement pour ces deux modules.
- Pour le `Mutex` Ocara : nécessite un mécanisme structurel (ex. `withLock(closure)` qui garantit le déverrouillage même en cas de `raise`, ou une intégration propre entre `longjmp` et les guards Rust) — non trivial.

## Fichiers clés

`runtime/src/sqlite.rs`, `runtime/src/mysql.rs`, `runtime/src/mutex.rs`, `src/builtins/mutex.rs`, `runtime/src/lib.rs` (`__ocara_fail`/`TryStack`).
