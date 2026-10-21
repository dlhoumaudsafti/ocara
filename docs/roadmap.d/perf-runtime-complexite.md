# Performances du runtime : complexités quadratiques et surcoûts constants

Audit du 2026-10-19. Mesures sur 12 cœurs chargés (min ou médiane de 3 à 5
exécutions) ; méthodes dans `docs/benchmarking.md` (test par doublement,
échantillonnage gdb, `strace -c`).

## Quadratique

1. **`map<K, V>` : chaque accès parcourt toute la map.** `OcaraMap` stocke un
   `Vec<(String, i64)>` (`runtime/src/containers.rs` ~l.17-22) ;
   `map_put`/`map_lookup` cherchent linéairement (`runtime/src/lib.rs`
   ~l.694-730) et `key_to_string` alloue une `String` par accès, même pour
   une clé `int`.

   | N insertions + N lectures | Ocara | Python |
   |---|---|---|
   | 10 000 | 0,23–0,32 s | — |
   | 40 000 | 2,5–5,3 s | — |
   | 100 000 | 18–21 s | 0,10 s |
   | 1 000 000 | > 120 s | 1,2 s |

   Touche aussi les maps issues de SQLite/MySQL, `JSON::decode`, les
   en-têtes HTTP. Piste : index de hachage à côté du `Vec` (ordre d'insertion
   conservé), clés `int` stockées en `i64`.
2. **`s = s + "x"` quadratique, trois copies par concaténation**
   (`__str_concat`, `lib.rs` ~l.428) : 10⁵ en 0,5 s, 4·10⁵ en 8,8 s, 10⁶ en
   67 s (Python : 0,026 s à 10⁶). Piste : ajout en place quand le compte de
   références vaut 1 ; `__str_build(n, parts…)` pour les gabarits (environ 4
   fois plus lents que les f-strings Python : 410–610 ns par gabarit à trois
   parties contre 135 ns).
3. **Collecteur de cycles : chaque collecte reparcourt le graphe vivant.**
   Tout `release` d'un objet de compte > 1 l'inscrit comme racine (un simple
   appel de méthode suffit) ; tous les 10 000 candidats, `collect_exclusive`
   (`runtime/src/rc.rs` ~l.527-551) parcourt tout le sous-graphe atteignable ;
   `for_each_child` clone le `Vec` des enfants. Graphe de 160 000 nœuds :
   0,7 s ; 320 000 : 4,2 s. Piste : ne jamais inscrire une valeur acyclique
   (classe sans champ référence, tableau `FLAG_RAW`, closure sans capture),
   seuil adaptatif en collecte exclusive, parcours sans copie.

## Surcoûts constants

4. **Appel de méthode** : ~26 ns (interface : ~50 ns) contre 1,4 ns pour une
   fonction libre — `self` est retenu puis relâché à chaque appel, avec
   atomiques SeqCst et passage par `possible_root`
   (`src/lower/builder.d/classes.rs` ~l.47-153, `src/lower/stmt.d/rc.rs`
   ~l.465-481). Une méthode Ocara est presque aussi lente que Python (37 ns).
5. **Chaque objet créé passe par le tampon du collecteur** (`init` retient
   puis relâche `self` à compte 2) : libération différée, 90–140 ns par
   `use P(i)`.
6. **`try`/`raise` : deux `rt_sigprocmask` par exception** (`setjmp` de glibc
   sauvegarde le masque) — utiliser `_setjmp`/`_longjmp`.
7. **`async` : un thread système par appel** (~30 µs/appel, 100 000
   `clone3` pour 100 000 appels) — pool de threads (`__task_spawn`,
   `runtime/src/task.rs`).
8. Spinlock CAS à chaque accès de tableau ou de map, même en mono-thread
   (~10 ns/opération).
9. `--release` ne fait que retirer les symboles (pas de niveau
   d'optimisation : Cranelift `opt_level=none`) ; les boucles serrées sont 2 à
   4 fois plus lentes que `gcc -O2`.

## Priorité / Complexité

**Haute** (complexité quadratique sur des usages courants) — **Structurel**
(map, collecteur), **Légère** (setjmp, concaténation en place).

## Fichiers clés

`runtime/src/containers.rs`, `runtime/src/lib.rs`, `runtime/src/rc.rs`,
`src/lower/builder.d/classes.rs`, `src/lower/stmt.d/rc.rs`,
`src/codegen/emit.d/emitter.rs`.
