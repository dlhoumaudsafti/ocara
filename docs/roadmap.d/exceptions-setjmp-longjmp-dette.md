# Dette transversale `setjmp`/`longjmp` — trois symptômes indépendants d'une même cause

## Terminé — Option B (diagnostic W04) ET Option A (généralisation) faites pour SQLite/MySQL/MariaDB

Voir §"Ce qui a été fait" plus bas. `Mutex`/`Thread` restent volontairement sans équivalent `withX` — voir §"Option A" pour la justification.

## Constat

Le mécanisme d'exceptions d'Ocara (`raise`/`try`/`on`) est implémenté en `setjmp`/`longjmp` (`src/lower/stmt.d/statements.d/exceptions.rs`) — pas de stack unwinding avec appel de destructeurs façon C++/Rust. `longjmp` saute par-dessus tout code de nettoyage intermédiaire. Cette même limitation architecturale ressurgit, documentée indépendamment à trois endroits différents du projet, comme si c'était trois bugs distincts alors que c'est une seule cause :

1. **`scoped`/`consumed` traversée par un `raise`** (`docs/EBNF.md` §9.2, `src/lower/stmt.d/ownership.rs:20-33`) — un `raise` qui traverse un `try` englobant fait fuir toutes les `scoped`/`consumed` encore vivantes dans les blocs traversés. Limite acceptée : fuite possible, jamais de corruption (rien d'autre ne peut aliaser cette mémoire).
2. **Générateur suspendu abandonné** (`docs/roadmap.d/langage-emit-iterable.md` §4, "Cas B — reporté") — un `raise` du consommateur d'un `for`/`message<T>` abandonne le générateur encore suspendu sans jamais le reprendre ni le libérer. Explicitement rattaché par la fiche elle-même à "même famille que la limitation déjà acceptée... pour un `scoped`/`consumed`".
3. **Mutex jamais déverrouillé** (`docs/builtins/Mutex.md`) — `lock()`/`unlock()` manuels laissent le mutex verrouillé pour toujours si un `raise` saute l'`unlock()`. Mitigé au niveau API par `m.withLock(f)` (lock + appel + unlock garanti même si `f()` raise), mais c'est un contournement ponctuel pour **ce seul builtin** — `SQLite`/`MySQL`/`MariaDB`/`Thread` (mêmes contraintes d'échappement que `Mutex`, voir §9.2 tableau EBNF) n'ont pas d'équivalent `withLock`.

Le motif se répète : chaque symptôme est découvert, documenté honnêtement, puis mitigé localement (un `withLock` ici, un "reporté" là) — jamais la cause commune. Rien ne garantit qu'un futur builtin possédant une ressource (`docs/adding-builtins.md`) ne réintroduise pas exactement le même trou, faute d'un mécanisme générique.

## Ce qui a été fait — Option B (diagnostic W04)

Nouveau module `src/sema/resource_raise.rs`, appelé depuis `TypeChecker::check_program` (`src/sema/typecheck.rs`), analyse dédiée (même patron que `crate::sema::escape` : une passe séparée sur tout le `Program`, pas entrelacée avec le checker statefull existant). Détecte qu'une `scoped`/`consumed` ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`/`HTTPRequest`/`HTTPResponse`/`Thread`) reste ouverte quand un `raise` plus loin dans le même bloc n'est protégé par aucun `try` local — nouveau warning **W04** (`docs/diagnostics.md`).

Volontairement conservateur (mêmes principes que E26/E28, voir la doc du module) — jamais de faux positif au prix de rater certains cas réels :
- un `raise` à l'intérieur d'un `try` LOCAL est assumé rattrapé (sans vérifier que ses `on` couvrent réellement la classe levée — hors périmètre) ;
- un `raise` dans un `on` HANDLER compte, lui, comme atteignant le bloc englobant (plus aucun `try` ne le protège) ;
- une finalisation (`.destroy()`/`.close()`/`.join()`/`.detach()`) en ligne droite avant le `raise` supprime l'avertissement pour cette ressource ;
- un `raise` atteignable via un `if`/`while`/`for`/`switch` imbriqué (sans son propre `try`) compte comme atteignant le bloc englobant ;
- aucune analyse interprocédurale — seul un `raise` textuel compte, pas un appel vers une fonction qui pourrait elle-même en lever un.

Vérifié : 7 tests unitaires Rust (`src/sema/tests/resource_raise.rs`) couvrant chaque cas de la doc du module ; validation manuelle sur 7 scénarios réalistes (`.oc`), tous corrects du premier coup ; **zéro faux positif** sur les 203 exemples existants du projet (aucun n'a jamais déclenché ce warning). `cargo test -p ocara --bin ocara` : 63 passed. `make regression` : 653 PASS, 0 FAIL (inchangé — un warning ne bloque pas la compilation, confirmé par un `--check` à exit code 0 malgré 3 warnings sur le fichier de test).

## Option A — généraliser le patron `withX` : FAIT pour SQLite/MySQL/MariaDB

Le diagnostic W04 rend le risque VISIBLE, il ne le CORRIGEAIT pas — la fuite/deadlock restait réelle si le développeur ignorait l'avertissement. Décision prise : généraliser le patron `withLock` (déjà fait pour `Mutex`) aux ressources où il s'applique proprement.

Reformulation du patron pour SQLite/MySQL/MariaDB : contrairement à `Mutex::withLock(f)` où le mutex existe déjà et est seulement CAPTURÉ par la closure (`nameless(): void {...}`, zéro paramètre), la connexion SQLite/MySQL n'existe pas encore avant l'appel — la ressource fraîchement créée est donc PASSÉE en paramètre à la closure (`nameless(db:SQLite): void {...}` / `nameless(db:MySQL): void {...}`), pas capturée. D'où deux nouvelles méthodes statiques plutôt qu'une méthode d'instance :

- **`SQLite::withOpen(path:string, f:Function<void(SQLite)>): void`** — ouvre, exécute `f(db)`, ferme systématiquement (`runtime/src/sqlite.rs`, `SQLite_withOpen`).
- **`MySQL::withConnect(host, user, password, database, f:Function<void(MySQL)>): void`** — connecte, exécute `f(db)`, ferme systématiquement (`runtime/src/mysql.rs`, `MySQL_withConnect`) ; `MariaDB::withConnect` forward vers la même implémentation (`MariaDB_withConnect`, symbole natif séparé — MariaDB n'est PAS un simple alias compilateur, voir `runtime/src/mysql.rs`).

Nouveau primitif runtime partagé, `run_closure_catching_with_arg(func_ptr, env_ptr, arg1) -> Result<i64,(i64,i64)>` (`runtime/src/lib.rs`, juste après `run_closure_catching`) : même mécanique `TRY_STACK`/`setjmp` que `run_closure_catching`, dupliquée volontairement plutôt que refactorisée (ajout pur à un nouvel endroit, risque minimal sur le chemin déjà testé), mais pour un appel de closure à UN paramètre (`fn(env_ptr, arg1) -> i64`) au lieu de zéro.

**Bug de compilateur découvert et corrigé en cours de route** : un paramètre de closure de type classe (`nameless(db:SQLite): void {...}`) n'était jamais enregistré dans `builder.var_class` lors du lowering (`src/lower/expr.d/nameless.rs`, `lower_nameless_fn`) — seules les variables CAPTURÉES l'étaient. Conséquence : `db.execute(...)` à l'intérieur de la closure ne résolvait pas la classe de `db`, retombait sur le fallback générique `_method_execute` (qui n'existe pas), et l'appel devenait un no-op silencieux — aucune erreur de compilation, aucun crash, juste aucune écriture réelle en base. Repéré par un test manuel (`db.query(...)` juste après `db.execute("CREATE TABLE...")` dans la MÊME closure retournait 0 ligne). Corrigé en enregistrant tout paramètre `Type::Named(cls)` dans `builder.var_class`, au même endroit que les captures — correctif général, pas spécifique à SQLite (bénéficie à tout futur closure-paramètre de type classe).

**`Mutex`/`Thread` : pas d'équivalent `withX` — décision assumée.** `Mutex::withLock` existe déjà (protège une section critique autour d'une ressource qui VIT plus longtemps que l'appel). `Thread` n'a pas de ressource "ouverte puis fermée" du même genre : son cycle de vie normal (`spawn` → `join`/`detach`) n'a pas de section critique à protéger, et un `withX` n'y apporterait rien de plus que ce que W04 signale déjà. Pas de nouveau trou laissé silencieux : W04 continue de couvrir ces deux types pour tout usage manuel restant.

Vérifié : nouveau test `.oc` dédié (`examples/tests/49_sqlite_with_open_raiseTest.oc`, 4 méthodes/6 assertions) — exécution nominale, propagation d'exception, **preuve de fermeture réelle** (une transaction `BEGIN` sans `COMMIT` laissée ouverte par la closure qui `raise` : une connexion suivante sur le même fichier ne voit PAS la ligne insérée, preuve que `close()` a bien déclenché le ROLLBACK automatique de SQLite), et propagation vers un `try` englobant à deux niveaux. `MySQL::withConnect`/`MariaDB::withConnect` : pas de serveur MySQL/MariaDB disponible dans cet environnement (même limitation que le reste du projet, voir `docs/roadmap.d/qualite-couverture-tests.md`) — usage ajouté à `examples/builtins/mysql.oc` (skip propre par `ci/regression.sh` en l'absence de serveur, testera réellement dès qu'un service CI existe), correction vérifiée par construction identique à `SQLite_withOpen` (même primitif `run_closure_catching_with_arg`, même correctif `var_class`) et par relecture croisée avec `Mutex_withLock`. `cargo test -p ocara --bin ocara` : 63 passed (inchangé — pas de nouveau test Rust dédié, cette zone du compilateur n'a jamais eu de couverture Rust unitaire, seulement des `.oc` de bout en bout, voir les trois tickets `langage-closure-*` clos). `make regression` : 659 PASS, 0 FAIL (+6 vs avant, le nouveau fichier `.oc`).

## Priorité / Complexité

**Terminé.** Option B (W04) et Option A (généralisation `withOpen`/`withConnect` pour SQLite/MySQL/MariaDB) toutes deux faites. **Complexité réelle : Légère** — un peu plus que prévu ("Simple" initialement estimé pour l'option A côté plomberie builtin) à cause du bug de compilateur latent sur `var_class` découvert en cours de route, mais circonscrit à une seule ligne de correctif une fois identifié.

## Fichiers clés

`src/sema/resource_raise.rs`, `src/sema/tests/resource_raise.rs`, `src/sema/error.rs` (`SemaWarning::ScopedResourceRaiseLeak`), `src/sema/typecheck.rs`, `docs/diagnostics.md` (W04), `runtime/src/lib.rs` (`run_closure_catching`/`run_closure_catching_with_arg`), `runtime/src/mutex.rs` (`Mutex_withLock`, patron d'origine), `runtime/src/sqlite.rs` (`SQLite_withOpen`), `runtime/src/mysql.rs` (`MySQL_withConnect`/`MariaDB_withConnect`), `src/builtins/sqlite.rs`, `src/builtins/mysql.rs`, `src/codegen/desc.d/sqlite.rs`, `src/codegen/desc.d/mysql.rs`, `src/lower/expr.d/nameless.rs` (correctif `var_class`), `examples/tests/49_sqlite_with_open_raiseTest.oc`, `examples/builtins/mysql.oc`, `docs/builtins/SQLite.md`, `docs/builtins/MySQL.md`, `src/lower/stmt.d/statements.d/exceptions.rs`, `src/lower/stmt.d/ownership.rs`, `docs/roadmap.d/langage-emit-iterable.md` §4, `docs/EBNF.md` §9.2.
