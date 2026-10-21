# Exception non rattrapée dans un `Thread` : arrêt du processus au lieu de `ThreadException`

**Clos le 2026-10-21.** `Thread_run` exécute la closure sous `run_closure_catching` : une exception non rattrapée n'arrête que le thread ; `join()` la relève en `ThreadException` 102 (« Uncaught exception in thread: <Type> : <message> »), un thread détaché la signale sur stderr. Tests : `examples/tests/102_thread_exceptionTest.oc` ; doc : `docs/builtins/Thread.md`.


Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

Thread.md (« Exceptions ») promet une `ThreadException` 102 au `join()` :

```ocara
t.run(nameless(): void { raise use MyErr("x", 7) })
try { t.join() } on e is ThreadException { … }     // le processus s'arrête (rc 1)
```

Même chose avec `detach()`, ou avec un `Convert::strToInt("zz")` dans le
thread. `Thread_run` (`runtime/src/thread.rs` ~l.107-114) appelle `f(env)`
directement, alors que `__task_spawn` utilise `run_closure_catching`.

## Priorité / Complexité

**Haute** (crash) — **Simple**.

## Fichiers clés

`runtime/src/thread.rs`, `runtime/src/lib.rs` (`run_closure_catching`),
`docs/builtins/Thread.md`.
