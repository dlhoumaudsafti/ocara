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

## ✅ `Mutex` Ocara — corrigé (`Mutex::withLock`)

L'API `lock()`/`unlock()` reste manuelle (pas de RAII côté langage) et donc toujours exposée au même risque si elle est utilisée directement — ce n'est pas retouché, par choix : ce sont des primitives bas niveau assumées comme telles (même discipline que `SQLite`/`MySQL` avant leur correctif : documenté, pas retiré).

Correctif additif : une nouvelle méthode `m.withLock(f:Function<void>) → void` (`Mutex_withLock`, `runtime/src/mutex.rs`) verrouille, exécute `f()` sous protection d'une frame try dédiée, puis déverrouille **systématiquement** — y compris si `f()` lève une exception. Le mécanisme réutilise directement l'infrastructure `setjmp`/`longjmp` déjà en place (`TRY_STACK`/`TryFrame`, `runtime/src/lib.rs`) via un nouvel helper interne, `run_closure_catching(func_ptr, env_ptr) -> Result<i64, (error_val, error_type)>` : il pousse sa propre frame et fait le `setjmp`, appelle la closure, et — si un `longjmp` la traverse — renvoie `Err((error_val, error_type))` au lieu d'appeler un handler (contrairement à `__ocara_try_exec`). `Mutex_withLock` déverrouille alors le mutex puis relance la MÊME exception via `__ocara_fail`, qui la fait continuer de se propager normalement (vers le `try/on` appelant s'il y en a un, ou termine le programme sinon) — exactement comme si `withLock` n'existait pas, à la différence près que le mutex n'est jamais laissé verrouillé.

**Vérifié** (voir `examples/builtins/mutex.oc` et `examples/tests/36_mutex_withlockTest.oc`, 8 assertions) :
- exécution nominale : la closure s'exécute, le mutex est déverrouillé après (prouvé par un `tryLock()` qui réussit juste après) ;
- exception dans la closure : rattrapée normalement par le `try/on` englobant, avec `message`/`code` intacts ;
- après une exception dans `withLock`, un `tryLock()` immédiat réussit (preuve directe qu'aucun deadlock ne subsiste) ;
- propagation à plusieurs niveaux de `try` imbriqués : fonctionne comme un `raise` ordinaire ;
- reproduction multi-thread manuelle (un thread A fait `raise` à l'intérieur d'un `withLock`, un thread B acquiert ensuite le même mutex sans bloquer) : passe, aucun hang.

`make regression` : 422 PASS / 0 FAIL (était 414 avant l'ajout des 8 assertions du nouveau test), aucune régression.

## Fichiers clés

`runtime/src/sqlite.rs`, `runtime/src/mysql.rs`, `runtime/src/mutex.rs` (`Mutex_withLock`), `src/builtins/mutex.rs` (déclaration `withLock`), `src/codegen/desc.d/mutex.rs` (`Mutex_withLock` dans `MUTEX_BUILTINS`), `runtime/src/lib.rs` (`__ocara_fail`/`TryStack`/`run_closure_catching`).
