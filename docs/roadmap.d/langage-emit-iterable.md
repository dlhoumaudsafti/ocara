# Instruction `emit` (générateurs, façon `yield` PHP) et type `message<T>`

## Demande

Ajouter une instruction `emit valeur` utilisable dans le corps d'une fonction/méthode, avec le même rôle que `yield` en PHP : elle **suspend** l'exécution de la fonction en produisant une valeur au consommateur, puis **reprend** exactement où elle s'était arrêtée au prochain élément demandé — sans réexécuter la fonction depuis le début et sans matérialiser tout le résultat en mémoire d'un coup (contrairement à `return array<T>`). Une fonction/méthode qui contient au moins un `emit` retourne un nouveau type, `message<T>`.

**Conception entièrement actée ci-dessous, prête pour implémentation** — ce chantier reste Massive (rien n'existe aujourd'hui, à aucun niveau), mais plus aucune question de conception n'est ouverte.

---

## Conception actée

### 1. Le type `message<T>`

- **Nom en minuscule**, comme `array<T>`/`map<K,V>` (les autres types génériques intégrés) — pas `Message<T>`/`Wrap<T>` (majuscule = réservé aux classes utilisateur/builtin) ni `iterable<T>` (jugé peu parlant). Cohérent lexicalement avec `emit` (« émettre ») : on émet un message, qui contient l'information.
- **`message<T>` est une étiquette de retour, jamais un type de premier ordre** : valable UNIQUEMENT comme type de retour déclaré d'une fonction/méthode qui contient elle-même au moins un `emit`. Jamais comme type de paramètre, jamais comme type de retour d'une fonction qui se contenterait de faire suivre (`return`) le `message<T>` d'une autre sans elle-même émettre — aucun transfert/forwarding à prendre en charge.
- **`message<T>` n'est JAMAIS nommable** — `var`/`scoped`/`consumed msg:message<int> = truc()` sont TOUS rejetés à la compilation. Il n'existe que comme résultat anonyme et immédiat d'un appel à une fonction/méthode contenant `emit`, consommé sur-le-champ (voir §2). Conséquence directe et importante : **`message<T>` ne rejoint PAS `OwnershipClass`, aucune nouvelle catégorie de possession n'est nécessaire** — ne pouvant jamais être lié à une variable, il ne peut jamais fuir, être réutilisé, passé en argument ailleurs, ni vivre au-delà d'une seule expression. Toutes les questions que `scoped`/`consumed`/`var` doivent résoudre pour Mutex/HTTPRequest/etc. (E17, E25, E26, E28...) ne se posent structurellement jamais ici.

### 2. Les trois façons de consommer un `message<T>`

- **`for x in truc() { }`** — itération complète, la forme "naturelle".
- **Consommation scalaire directe, dans n'importe quelle position qui attend un `T`** (affectation, argument d'appel, expression) :
  ```ocara
  function truc(): message<int> { emit 1 }
  var value:int = truc()

  function truc3(): message<string> { emit "salut" }
  IO::writeln(truc3())   // consommée directement en position d'argument
  ```
  **Rejetée à la compilation SAUF preuve statique qu'au plus un seul `emit` est atteignable.** Règle précise : **`emit` dans une boucle reste du Ocara parfaitement valide** (c'est le cas d'usage principal du chantier) — la restriction porte UNIQUEMENT sur la consommation scalaire directe. **Aucun `emit` atteignable à l'intérieur d'un corps de boucle** (`while`/`for`) disqualifie la consommation scalaire ; le branchement simple (`if`/`elseif`/`else`, `switch`) reste autorisé tant que chaque chemin ne traverse qu'un seul `emit` — `if cond { emit 1 } else { emit 2 }` est donc consommable directement, alors qu'un `emit` dans une boucle ne l'est jamais (même prouvable comme ne s'exécutant qu'une fois — analyse volontairement simple/conservatrice). Une fonction disqualifiée reste consommable via `for` ou `Array::fromMessage`.
- **`Array::fromMessage(message<T>) → array<T>`** — draine TOUS les `emit` (exécute la machine à états jusqu'au bout) dans un `array<T>` neuf. Aucune restriction sur le nombre d'`emit` : c'est l'échappatoire pour le cas multi-émissions.
  ```ocara
  function truc(): message<int> {
      emit use MaClasseEntity("Jean", "Dupond", 56)
      emit use MaClasseEntity("John", "Doe", 35)
  }
  var entities:array<MaClasseEntity> = Array::fromMessage(truc())
  ```

### 3. Stratégie de suspension/reprise : machine à états à la compilation

Choisi explicitement pour la performance (pas de thread OS dédié par générateur). Chaque fonction contenant `emit` est réécrite en une structure qui mémorise son point de reprise (un entier d'état) et ses variables locales survivant à travers les `emit` (promues sur le tas, comme les captures de closure le sont déjà). C'est le morceau le plus dur du chantier — voir §5 ci-dessous.

**Point clé pour la suite** : reprendre un générateur n'est PAS un retour dans une pile suspendue — c'est un **nouvel appel de fonction normal** qui regarde l'état sauvegardé, saute au bon endroit, et rend la main normalement au prochain `emit`. La pile est entièrement redéroulée entre deux reprises. C'est ce qui rend le Cas A (§4) réparable.

### 4. Interaction avec `raise`/`try` — les deux cas tranchés séparément

- **Cas A — `emit` à l'intérieur d'un `try`, dans le générateur lui-même : PRIS EN CHARGE.**
  ```ocara
  function truc(): message<int> {
      try {
          emit 1
          emit 2
      } on e {
          IO::writeln("erreur attrapée")
      }
  }
  ```
  Solution retenue : puisque chaque reprise repart d'une pile fraîche et valide (voir §3), le compilateur suit, dans l'état sauvegardé du générateur, la liste des `try` encore "ouverts" (avec la bonne gestion de l'imbrication) à chaque point de suspension, et **rejoue les `setjmp` correspondants** (repousse les `TryFrame` nécessaires) au tout début de chaque reprise, avant de sauter au point sauvegardé. Coût quasi nul (`setjmp` est bon marché, rejoué seulement pour les `try` réellement actifs à cet endroit) — cohérent avec l'objectif de performance.

- **Cas B — un `raise` du CONSOMMATEUR abandonne un générateur suspendu : REPORTÉ, à garder en mémoire.**
  ```ocara
  function compteur(): message<int> {
      var i:int = 0
      while i smaller 10 {
          emit i
          i = i + 1
      }
  }
  try {
      for x in compteur() {
          if x equal 3 { raise "boom" }
          IO::writeln(x)
      }
  } on e {
      IO::writeln("attrapé")
  }
  ```
  `raise` fait un `longjmp` qui saute par-dessus le `for` (et donc le générateur encore suspendu à `x == 3`) sans jamais le reprendre ni le nettoyer. `longjmp` saute par-dessus le code de nettoyage normal — **même famille que la limitation déjà acceptée dans ce projet** pour un `scoped`/`consumed` traversé par un `raise` (documentée, non corrigée : fuite possible, jamais de corruption). Une vraie correction demanderait un mécanisme de déroulement de pile avec destructeurs (comme les exceptions C++/Rust), pas `setjmp`/`longjmp` — changement d'architecture hors de proportion pour ce chantier. **Décision : reporté, mais à documenter explicitement** (voir §6 "Limite connue et acceptée") plutôt que traité en silence.

### 5. Libération du frame suspendu (mécanique, locale aux 3 formes de consommation — pas un système de possession général)

- **`for` qui va jusqu'au bout** : libération normale, symétrique à la fin d'un `for` classique.
- **`for` interrompu tôt (`break`, `return` anticipé)** : le cas le plus dur du chantier — le frame encore suspendu (locales survivantes, éventuelles ressources internes) doit être détruit proprement, sans fuite ni double-libération. À traiter avec le même soin que `emit_early_exit_drops`/`block_scope_stack` (`src/lower/stmt.d/ownership.rs`), qui résout déjà un problème structurellement proche.
- **Consommation scalaire directe** : prend une valeur puis détruit immédiatement le reste (jamais démarré au-delà du premier état, pas de frame "en pause" à nettoyer).
- **`Array::fromMessage`** : va jusqu'au bout par définition, se comporte comme le premier cas de `for` ci-dessus.

### 6. Limite connue et acceptée (à documenter dans l'EBNF/diagnostics une fois implémenté)

Un `raise` déclenché par le code CONSOMMATEUR (pas le générateur lui-même) alors qu'un `message<T>` est encore suspendu (via `for`) abandonne ce générateur sans nettoyage — fuite possible (jamais de corruption mémoire), même famille que la limitation déjà acceptée pour `scoped`/`consumed` traversée par un `raise`. Non corrigé par ce chantier ; à revisiter uniquement si ce projet se dote un jour d'un vrai mécanisme de déroulement de pile avec destructeurs.

---

## Liste des tâches d'implémentation

Dans un ordre logique de dépendance :

1. **Parsing** : mot-clé `emit`, nouveau `Stmt::Emit` (`src/parsing/ast.d/statements.rs`), nouveau `Type::Message` valable uniquement en position de retour (`src/parsing/ast.d/types.rs`). Grammaire dans `docs/EBNF.md`.
2. **Sema — typage et validations** (`src/sema/typecheck.rs`) :
   - Type de retour `message<T>` inféré/vérifié pour toute fonction/méthode contenant au moins un `emit`.
   - Rejet de `var`/`scoped`/`consumed message<T>` (jamais nommable).
   - Rejet de `message<T>` comme type de paramètre ou de retour d'une fonction sans `emit` propre.
   - Analyse statique "au plus un `emit` atteignable hors boucle" pour autoriser la consommation scalaire directe — nouveau diagnostic si violée.
3. **Lowering — transformation en machine à états** (`src/lower/`) : identifier les points de suspension (chaque `emit`), calculer les variables locales survivantes (promotion sur le tas, même patron que les captures de closure), régénérer le corps de fonction en structure pilotée par un entier d'état + point d'entrée unique qui reprend au bon endroit.
4. **Lowering — les 3 formes de consommation** : `for x in truc()`, consommation scalaire directe (déballage automatique en position `T`), `Array::fromMessage`. Génération des appels de libération adaptés à chaque forme (voir §5), y compris `break`/`return` anticipé dans un `for`.
5. **Lowering — Cas A (`emit` dans un `try`)** : suivi des `try` actifs par point de suspension dans l'état sauvegardé, rejeu des `setjmp`/`TryFrame` nécessaires à chaque reprise (voir §4).
6. **Runtime** : `Array::fromMessage` (`src/builtins/array.rs`, `runtime/src/lib.rs`), et toute primitive d'exécution nécessaire à la machine à états si le lowering seul n'y suffit pas.
7. **Documentation** : `docs/EBNF.md` (nouvelle syntaxe), `docs/workflow-compilation.md`, `docs/adding-types.md`, mention de la limite connue du Cas B (§6) une fois le reste implémenté.
8. **Tests de régression** : nouveau fichier `examples/tests/`, couvrant au minimum : `emit` simple + `for`, consommation scalaire directe (cas autorisé et cas rejeté à la compilation), `Array::fromMessage`, `break`/`return` anticipé dans un `for`, `emit` dans un `try` (Cas A), imbrication de générateurs.

## Fichiers clés (probables)

`src/parsing/` (lexer/parser), `src/parsing/ast.d/statements.rs` (`Stmt::Emit`), `src/parsing/ast.d/types.rs` (`Type::Message`), `src/sema/typecheck.rs` (typage, validations, diagnostic "au plus un emit hors boucle"), `src/lower/` (transformation en machine à états), `src/lower/stmt.d/ownership.rs` (`emit_early_exit_drops`/`block_scope_stack`, pattern réutilisé pour la destruction du frame et le rejeu des `setjmp`), `src/builtins/array.rs`/`runtime/src/lib.rs` (`Array::fromMessage`), `docs/EBNF.md`, `docs/workflow-compilation.md`, `docs/adding-types.md`.
