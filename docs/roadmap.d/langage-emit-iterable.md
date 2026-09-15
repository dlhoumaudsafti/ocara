# Instruction `emit` (générateurs, façon `yield` PHP) et type `iterable`

## Demande

Ajouter une instruction `emit valeur` utilisable dans le corps d'une fonction/méthode, avec le même rôle que `yield` en PHP : elle **suspend** l'exécution de la fonction en produisant une valeur au consommateur (typiquement un `for x in maFonction() { }`), puis **reprend** exactement où elle s'était arrêtée au prochain élément demandé — sans réexécuter la fonction depuis le début et sans matérialiser tout le résultat en mémoire d'un coup (contrairement à `return array<T>`). Une fonction qui contient au moins un `emit` retourne un nouveau type, `iterable` (probablement `iterable<T>`, à trancher).

## État actuel

**Rien de tout ça n'existe aujourd'hui, à aucun niveau :**
- Aucun mot-clé `emit`/`yield` dans le lexer/parser (`src/parsing/`).
- Aucun type `iterable`/`Iterable<T>` dans `Type` (`src/parsing/ast.d/types.rs`) — seuls `Array`/`Map`/`Generic` existent comme conteneurs.
- Aucune notion de suspension/reprise d'exécution nulle part dans le compilateur ou le runtime : pas de coroutine, pas de continuation, pas de machine à états généré pour une fonction. `for i in expr`/`for k => v in expr` (`Stmt::ForIn`/`Stmt::ForMap`) itèrent uniquement sur un `array`/`map` déjà pleinement construit en mémoire — le modèle d'exécution est strictement "une fonction s'exécute du début à la fin en une seule fois", cranelift-native pur, sans aucune primitive de suspension.
- Le seul mécanisme de concurrence existant (`Thread`, `pthread` réel) n'a pas de notion de "reprendre exactement au point d'arrêt avec l'état local intact" — un thread OS a sa propre pile, mais rien dans ce langage ne l'utilise aujourd'hui pour simuler un générateur.

## Ampleur et questions de conception à trancher avant de commencer

C'est une fonctionnalité de langage entièrement à construire, pas un correctif — Massive, et structurante (touche parsing/AST, sémantique/typage, lowering, codegen ET runtime) :

1. **Stratégie d'implémentation pour la suspension/reprise** — deux familles possibles, à choisir explicitement :
   - **Thread OS dédié par générateur** (réutilise l'infrastructure `pthread` déjà en place pour `Thread`/`Mutex`) : la fonction génératrice tourne sur son propre thread, synchronisée avec le consommateur via une paire de sémaphores/condvars (le thread producteur bloque après chaque `emit` jusqu'à ce que le consommateur redemande une valeur). Simple à raisonner (aucune transformation de l'IR), mais un coût réel par générateur (pile de thread OS entière, changement de contexte à chaque valeur) — à évaluer si c'est acceptable pour un usage "boucle sur beaucoup de petits éléments".
   - **Transformation en machine à états à la compilation** (façon Rust/C#) : chaque fonction contenant `emit` est réécrite en une structure qui mémorise son point de reprise (un entier d'état) et ses variables locales survivant à travers les `emit` (promues dans une structure, comme les captures de closure le sont déjà sur le tas). Beaucoup plus léger à l'exécution, mais demande un vrai chantier de compilation (identifier les points de suspension, calculer quelles variables locales doivent survivre, régénérer le corps de fonction en conséquence) — un ordre de grandeur plus complexe que la première option.
2. **Le type `iterable`** — `iterable<T>` générique (dans quelle mesure ressemble-t-il à `array<T>`/`Generic` existant côté typage ?), consommable uniquement par `for`, ou aussi convertible en `array<T>` explicitement (`Array::fromIterable(...)`) ? Un `iterable` est-il à usage unique (un seul passage, comme un itérateur PHP/Rust) ou ré-itérable ?
3. **Ownership/mémoire** — un générateur "en pause" retient un état vivant (variables locales, éventuellement des ressources `scoped`/`consumed` capturées) entre deux appels : comment ce chantier s'articule avec l'analyse d'échappement existante (`src/sema/escape.rs`) et la libération de fin de bloc — un générateur jamais entièrement consommé (le consommateur arrête la boucle avant la fin) doit-il/peut-il libérer proprement ce qu'il retient ?
4. **Interaction avec `raise`/`try`** — un `emit` à l'intérieur d'un `try`, ou une exception levée entre deux reprises, n'a aucun précédent dans ce compilateur (le mécanisme `setjmp`/`longjmp` actuel suppose une pile d'exécution linéaire).

## Fichiers clés (probables, à confirmer une fois la stratégie choisie)

`src/parsing/` (lexer/parser, nouveau mot-clé + type), `src/parsing/ast.d/statements.rs` (nouveau `Stmt::Emit`), `src/parsing/ast.d/types.rs` (`Type::Iterable`), `src/sema/typecheck.rs` (type de retour d'une fonction contenant `emit`), `src/lower/` (stratégie de suspension choisie), `runtime/src/lib.rs`/`runtime/src/thread.rs` (si stratégie thread OS), `docs/EBNF.md` (nouvelle syntaxe), `docs/workflow-compilation` (Comprendre la mécanique et le workflow de compilation), `docs/adding-types` (Ajouter un nouveau type)
