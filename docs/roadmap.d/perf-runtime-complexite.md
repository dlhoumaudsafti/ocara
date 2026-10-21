# Performances du runtime : surcoûts constants restants

Audit du 2026-10-19, mis à jour le 2026-10-21. Mesures et méthodes dans
`docs/benchmarking.md` (test par doublement, échantillonnage gdb,
`strace -c`).

## Surcoûts constants

1. **Méthode qui appelle du code : `self` retenu puis relâché** (~20 ns
   par appel, interface ~50 ns). Une méthode dont le corps ne contient ni
   appel, ni `use`, ni écriture dans un autre objet ou un conteneur s'en
   passe déjà (`src/lower/builder.d/self_borrow.rs`, ~0,5 ns). Piste :
   prouver qu'un appel ne peut pas libérer le receveur (receveur tenu par
   une locale non capturée de l'appelant), ou des retenues non atomiques
   tant qu'un seul thread Ocara tourne.
2. **Objet dont `init` appelle du code** : même retenue de `self`, et
   l'objet passe par le tampon du collecteur à sa création.
3. **Gabarits** : une concaténation par partie (`__str_concat`), 410–610 ns
   pour trois parties contre 135 ns en Python. Piste : `__str_build(n,
   parts…)`, une seule allocation.
4. **`async` : un thread système par appel** (~30 µs/appel, 100 000
   `clone3` pour 100 000 appels) — pool de threads (`__task_spawn`,
   `runtime/src/task.rs`).
5. Spinlock CAS à chaque accès de tableau ou de map, même en mono-thread
   (~10 ns/opération).
6. `--release` ne fait que retirer les symboles (pas de niveau
   d'optimisation : Cranelift `opt_level=none`) ; les boucles serrées sont 2 à
   4 fois plus lentes que `gcc -O2`.

## Priorité / Complexité

**Moyenne** (plus de complexité quadratique : surcoûts constants) —
**Légère** (gabarits, spinlock) à **Structurel** (retenue de `self`, pool de
threads).

## Fichiers clés

`src/lower/builder.d/self_borrow.rs`, `src/lower/stmt.d/rc.rs`,
`runtime/src/rc.rs`, `runtime/src/task.rs`, `runtime/src/containers.rs`,
`src/codegen/emit.d/emitter.rs`.
