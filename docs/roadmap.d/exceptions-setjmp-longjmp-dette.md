# Dette transversale `setjmp`/`longjmp` — trois symptômes indépendants d'une même cause

## Constat

Le mécanisme d'exceptions d'Ocara (`raise`/`try`/`on`) est implémenté en `setjmp`/`longjmp` (`src/lower/stmt.d/statements.d/exceptions.rs`) — pas de stack unwinding avec appel de destructeurs façon C++/Rust. `longjmp` saute par-dessus tout code de nettoyage intermédiaire. Cette même limitation architecturale ressurgit, documentée indépendamment à trois endroits différents du projet, comme si c'était trois bugs distincts alors que c'est une seule cause :

1. **`scoped`/`consumed` traversée par un `raise`** (`docs/EBNF.md` §9.2, `src/lower/stmt.d/ownership.rs:20-33`) — un `raise` qui traverse un `try` englobant fait fuir toutes les `scoped`/`consumed` encore vivantes dans les blocs traversés. Limite acceptée : fuite possible, jamais de corruption (rien d'autre ne peut aliaser cette mémoire).
2. **Générateur suspendu abandonné** (`docs/roadmap.d/langage-emit-iterable.md` §4, "Cas B — reporté") — un `raise` du consommateur d'un `for`/`message<T>` abandonne le générateur encore suspendu sans jamais le reprendre ni le libérer. Explicitement rattaché par la fiche elle-même à "même famille que la limitation déjà acceptée... pour un `scoped`/`consumed`".
3. **Mutex jamais déverrouillé** (`docs/builtins/Mutex.md`) — `lock()`/`unlock()` manuels laissent le mutex verrouillé pour toujours si un `raise` saute l'`unlock()`. Mitigé au niveau API par `m.withLock(f)` (lock + appel + unlock garanti même si `f()` raise), mais c'est un contournement ponctuel pour **ce seul builtin** — `SQLite`/`MySQL`/`MariaDB`/`Thread` (mêmes contraintes d'échappement que `Mutex`, voir §9.2 tableau EBNF) n'ont pas d'équivalent `withLock`.

Le motif se répète : chaque symptôme est découvert, documenté honnêtement, puis mitigé localement (un `withLock` ici, un "reporté" là) — jamais la cause commune. Rien ne garantit qu'un futur builtin possédant une ressource (`docs/adding-builtins.md`) ne réintroduise pas exactement le même trou, faute d'un mécanisme générique.

## Ce qui est demandé

Ne pas nécessairement réécrire tout le mécanisme d'exceptions en stack unwinding réel (ampleur reconnue "hors de proportion" par `langage-emit-iterable.md` elle-même) — mais choisir consciemment entre deux options plutôt que de laisser chaque nouveau builtin redécouvrir le problème :

- **Option A (généraliser la mitigation existante)** : étendre le patron `withLock` à un mécanisme générique au niveau du langage — une forme de `defer`/`finally` ou un helper systématique pour toute ressource `scoped` (`SQLite`/`MySQL`/`MariaDB`/`Thread`), pas seulement `Mutex`. Coût mesuré (un seul builtin déjà fait), bénéfice immédiat sur les ressources restantes.
- **Option B (rendre le risque visible au lieu de silencieux)** : à défaut de corriger, un diagnostic de compilation qui détecte qu'une `scoped`/`consumed` `Resource`/`Thread` reste vivante à l'intérieur d'un `try` sans passer par un mécanisme de nettoyage garanti — transforme une fuite/deadlock silencieux en avertissement explicite à la compilation, cohérent avec l'esprit des diagnostics E17-E27 déjà existants (`docs/diagnostics.md`).

Ces deux options ne s'excluent pas : B est un filet de sécurité utile même si A est fait.

## Priorité / Complexité

**Priorité Haute** — c'est une limitation transversale qui touche potentiellement tout futur builtin à ressource, pas un cas isolé ; la roadmap actuelle la traite comme trois "reportés" séparés alors qu'elle est unique. **Complexité : Dangereuse** — touche le mécanisme d'exceptions, zone sensible où une erreur peut casser silencieusement (comme documenté dans `src/lower/stmt.d/statements.d/exceptions.rs`) ; l'option B (diagnostic) est nettement moins risquée que l'option A (nouveau mécanisme de nettoyage) et peut être traitée en premier.

## Fichiers clés

`src/lower/stmt.d/statements.d/exceptions.rs`, `src/lower/stmt.d/ownership.rs`, `src/lower/builder.d/message_gen.rs`, `runtime/src/mutex.rs` (`Mutex_withLock`), `docs/roadmap.d/langage-emit-iterable.md` §4, `docs/builtins/Mutex.md`, `docs/EBNF.md` §9.2.
