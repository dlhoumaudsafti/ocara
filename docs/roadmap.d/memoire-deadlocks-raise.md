# Blocages (deadlocks) provoqués par `raise` traversant un verrou tenu

## ✅ SQLite/MySQL/MariaDB — corrigé

## Mécanisme en cause

`raise` est implémenté via `setjmp`/`longjmp` (runtime/src/lib.rs), qui saute par-dessus les frames Rust sans exécuter leurs destructeurs (`Drop`). `SQLite_execute`/`SQLite_query`/`SQLite_queryOne` (et les équivalents MySQL) prenaient un verrou (`db.conn.lock()`/`db.pool.lock()`) puis, en cas d'erreur SQL, appelaient `throw_sqlite_exception`/`throw_mysql_exception` → `longjmp`. Le `MutexGuard` Rust ne voyait jamais son `Drop` s'exécuter → le mutex restait verrouillé indéfiniment → toute requête suivante sur la même connexion se bloquait.

**Confirmé par reproduction** (pas seulement théorique) :

```ocara
var db:SQLite = SQLite::open(":memory:")
try {
    db.execute("THIS IS NOT VALID SQL")
} on e {
    IO::writeln("caught first error (expected)")
}
db.execute("CREATE TABLE t (x INT)")   // bloque indéfiniment avant ce correctif
```

## Correctif

Pour chacune des 5 fonctions concernées (`SQLite_execute`/`query`/`queryOne`, `MySQL_execute`/`query`/`queryOne` — `MariaDB_*` sont de simples alias de `MySQL_*`), toute la logique qui tient le verrou est déplacée dans une closure locale retournant un `Result<_, String>` : le `MutexGuard` (et, pour SQLite, `Statement`/`Rows` qui en empruntent la durée de vie) se relâche alors normalement (vrai `Drop` Rust) à la sortie de la closure — succès ou échec — **avant** que le code appelant n'atteigne le `throw_*_exception` correspondant. Le verrou n'est donc plus jamais tenu au moment du `longjmp`.

Approche alternative écartée : `drop(guard)` explicite juste avant chaque `throw_*` dans les branches d'erreur existantes — se heurte au borrow-checker dès qu'une valeur retournée (`Statement`/`Rows` chez SQLite) porte la durée de vie de la connexion, y compris dans une branche qui ne l'utilise plus (limitation connue de l'inférence de régions de Rust, pas spécifique à ce code). La restructuration en closure contourne le problème proprement, sans `unsafe` supplémentaire.

Vérifié : la reproduction ci-dessus réussit maintenant (`CREATE TABLE` s'exécute normalement au lieu de bloquer). `make regression` sans régression (exemples `sqlite.oc`/`mysql.oc` — ce dernier `SKIP`é faute de serveur MySQL local disponible dans cet environnement, comme déjà documenté ailleurs ; les deux compilent et suivent la même restructuration).

## `Mutex` Ocara — toujours ouvert

L'API `lock()`/`unlock()` est manuelle (pas de RAII côté langage, `src/builtins/mutex.rs`). Un `raise` entre les deux saute l'`unlock()` → mutex jamais déverrouillé → tous les threads en attente bloquent indéfiniment. Nécessite un mécanisme structurel (ex. `Mutex::withLock(closure)` qui garantirait le déverrouillage même en cas de `raise`, ou une intégration propre entre `longjmp` et un futur RAII côté langage) — non traité dans cette passe (portée volontairement limitée à SQLite/MySQL/MariaDB, où le correctif est ponctuel et sans changement d'API visible).

## Fichiers clés

`runtime/src/sqlite.rs`, `runtime/src/mysql.rs`, `runtime/src/mutex.rs` (non touché, voir ci-dessus), `src/builtins/mutex.rs` (non touché), `runtime/src/lib.rs` (`__ocara_fail`/`TryStack`).
