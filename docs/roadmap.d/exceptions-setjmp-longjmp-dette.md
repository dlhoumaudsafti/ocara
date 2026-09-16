# Dette transversale `setjmp`/`longjmp` — trois symptômes indépendants d'une même cause

## Option B faite (diagnostic W04) — option A (généraliser `withLock`) en suspens, décision demandée

Voir §"Ce qui a été fait" plus bas.

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

## Option A — généraliser `withLock` : PAS entreprise, décision demandée

Le diagnostic W04 rend le risque VISIBLE, il ne le CORRIGE pas — la fuite/deadlock reste réelle si le développeur ignore l'avertissement (un warning, pas une erreur, par choix délibéré : contrairement à E18/E19/E26, il n'y a pas toujours de correction locale évidente, et le rendre bloquant casserait potentiellement du code existant qui accepte consciemment ce risque). Étendre le patron `withLock` (lock + appel + unlock garanti même si `f()` raise, déjà fait pour `Mutex` — voir `docs/builtins/Mutex.md`) à `SQLite`/`MySQL`/`MariaDB`/`Thread` fermerait le trou à la racine pour ces types, plutôt que de se reposer sur la discipline du développeur à chaque site d'usage signalé par W04.

**Décision demandée** : généraliser `withLock` maintenant (coût mesuré : un seul builtin déjà fait, sert de patron direct) vs s'arrêter à W04 pour l'instant (le risque est maintenant visible et documenté, pas silencieux) ? Pas tranché ici.

## Priorité / Complexité

**Option B : ✅ Terminée.** Option A : **non tranchée**, ticket laissé ouvert en Priorité Haute jusqu'à décision — la dette architecturale elle-même (setjmp/longjmp sans unwinding) reste entière, seulement rendue visible. **Complexité : Simple** pour l'option B faite (isolée dans un nouveau module, aucune modification du mécanisme d'exceptions lui-même) — contrairement à l'estimation initiale ("Dangereuse"), qui s'appliquait surtout à l'option A (toujours non tentée). **Complexité : Structurel** pour l'option A si retenue.

## Fichiers clés

`src/sema/resource_raise.rs` (fait), `src/sema/tests/resource_raise.rs` (fait), `src/sema/error.rs` (`SemaWarning::ScopedResourceRaiseLeak`), `src/sema/typecheck.rs` (câblage), `docs/diagnostics.md` (W04, fait), `src/lower/stmt.d/statements.d/exceptions.rs`, `src/lower/stmt.d/ownership.rs`, `src/lower/builder.d/message_gen.rs`, `runtime/src/mutex.rs` (`Mutex_withLock`, patron pour l'option A), `docs/roadmap.d/langage-emit-iterable.md` §4, `docs/builtins/Mutex.md`, `docs/EBNF.md` §9.2.
