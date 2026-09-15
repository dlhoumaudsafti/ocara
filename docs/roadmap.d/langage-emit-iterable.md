# Instruction `emit` (générateurs, façon `yield` PHP) et type `message<T>`

## Demande

Ajouter une instruction `emit valeur` utilisable dans le corps d'une fonction/méthode, avec le même rôle que `yield` en PHP : elle **suspend** l'exécution de la fonction en produisant une valeur au consommateur, puis **reprend** exactement où elle s'était arrêtée au prochain élément demandé — sans réexécuter la fonction depuis le début et sans matérialiser tout le résultat en mémoire d'un coup (contrairement à `return array<T>`). Une fonction/méthode qui contient au moins un `emit` retourne un nouveau type, `message<T>`.

**✅ Chantier terminé.** Les 8 étapes d'implémentation sont faites et vérifiées (`make regression` 552 + 49 PASS, 0 FAIL, 34 assertions dédiées dans `examples/tests/42_emit_generatorTest.oc`) : parsing, sema, machine à états, `for`/scalaire direct/`Array::fromMessage`, Cas A (`emit` dans un `try`, y compris imbriqué), documentation (EBNF, workflow-compilation, adding-types). Seule limite restante, documentée et acceptée : Cas B (`raise` du consommateur qui abandonne un générateur suspendu) et fuite du frame sur `return`/`raise` anticipé depuis un `for` — voir §6.

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

- **Cas A — `emit` à l'intérieur d'un `try`, dans le générateur lui-même : ✅ IMPLÉMENTÉ ET VÉRIFIÉ.**
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
  Solution retenue et implémentée : puisque chaque reprise repart d'une pile fraîche et valide (voir §3), `__resume` suit, dans l'état sauvegardé du générateur, la liste des `try` encore "ouverts" (avec la bonne gestion de l'imbrication) à chaque point de suspension, et **rejoue les `setjmp` correspondants** (repousse les `TryFrame` nécessaires) au tout début de chaque reprise, avant de sauter au point sauvegardé. `try` est lowered INLINE dans `__resume` (pas en fonction séparée comme le `lower_try` normal) — voir `crate::lower::builder::message_gen::lower_try_in_generator`. Deux nouvelles primitives runtime : `__ocara_try_enter`/`__ocara_try_exit` (push/pop du `TryFrame`, `setjmp` lui-même appelé DIRECTEMENT depuis le code généré, jamais via un wrapper Rust — seule façon de capturer la frame de `__resume` elle-même). Le mécanisme `raise`/`__ocara_fail`/`TRY_STACK` existant n'a pas été modifié : un générateur-`try` s'y insère comme n'importe quel autre `try`, donc un `raise` déclenché par une fonction APPELÉE depuis le `try` (pas seulement un `raise` textuel) est correctement rattrapé, et l'imbrication (`try` dans `try`, dans un générateur) respecte la sémantique LIFO normale. Vérifié par 24 assertions dédiées (`examples/tests/42_emit_generatorTest.oc`) + `make regression` intégral (542 + 49 PASS, 0 FAIL).

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

### 6. Limites connues et acceptées

- Un `raise` déclenché par le code CONSOMMATEUR (pas le générateur lui-même) alors qu'un `message<T>` est encore suspendu (via `for`) abandonne ce générateur sans nettoyage — fuite possible (jamais de corruption mémoire), même famille que la limitation déjà acceptée pour `scoped`/`consumed` traversée par un `raise`. Non corrigé par ce chantier ; à revisiter uniquement si ce projet se dote un jour d'un vrai mécanisme de déroulement de pile avec destructeurs.
- **(Implémentation, Étape 4)** Un `return` anticipé (sans valeur — seul cas valable dans un générateur) exécuté DANS le corps d'un générateur alors qu'il est consommé via `for`, comme un `raise` DEPUIS le corps du `for`, saute directement hors de la fonction consommatrice sans jamais atteindre le point de fusion où le frame est libéré (`merge_bb`) — même fuite que le point ci-dessus, jamais de corruption. `break` n'est PAS concerné (même bloc cible que l'épuisement naturel, donc déjà correctement nettoyé). Non corrigé pour cette étape.
- ~~(Implémentation, Étape 5) `emit` à l'intérieur d'un `try` rejeté à la compilation~~ — **implémenté**, voir §4 Cas A ci-dessus.

---

## Liste des tâches d'implémentation

Dans un ordre logique de dépendance :

1. **✅ Parsing** — `TokenKind::Emit`/`TMessage`, `Stmt::Emit`, `Type::Message(Box<Type>)`, `parse_emit()`, `message<T>` dans `parse_type_base()`. `emit`/`message` restent utilisables comme identifiants ordinaires hors position de mot-clé (`eat_ident()` + primaires d'expression), indispensable pour `ui.emit(...)` (Tauri) et `Exception.message`/`var message:...` (très répandu). Tous les sites de match exhaustif mis à jour (monomorph, render_file, runtime_expand, IrType::from_ast, captures, ownership, escape, typecheck). Grammaire EBNF pas encore mise à jour (fait au §7).
2. **✅ Sema — typage et validations** (`src/sema/typecheck.rs`, `src/sema/message_emit.rs`, `src/sema/symbols.d/`) :
   - Nouveaux diagnostics E30–E34 : `message<T>` non nommable (`var`/`scoped`/`consumed`) ; interdit comme type de paramètre (fonctions/méthodes ET constructeurs) ; retour `message<T>` sans `emit` atteignable dans le corps ; `emit` hors d'une fonction `message<T>` ; consommation scalaire directe interdite si `emit` atteignable dans une boucle (`FuncSig.message_emit_in_loop`, calculé une fois à l'enregistrement des symboles via `crate::sema::message_emit::analyze_emit`).
   - `types_compat` déballe `message<T>` trouvé vers `T` (consommation scalaire — `var`/`const`/`return`/argument de fonction libre ou d'appel statique) ; `Stmt::ForIn` accepte `message<T>` sans aucune restriction (toujours valable, y compris avec `emit` en boucle).
   - Vérifié manuellement (var/const scalaire, `for`, appel statique type `IO::writeln(...)`, méthode d'instance `obj.truc()`, branchement `if`/`else` autorisé, boucle correctement rejetée) + `make regression` intégral repassé au vert.
   - Non couvert (accepté comme limite mineure, hors du périmètre discuté) : paramètres de closures (`Expr::Nameless`), `Array::fromMessage` (prévu étape 6 : bypass dédié de cette même restriction).
3. **✅ Lowering — transformation en machine à états** (`src/lower/builder.d/message_gen.rs`) :
   - Chaque fonction/méthode `emit`-contenante devient DEUX `IrFunction` séparées : `<nom>__new(params...) -> Ptr` (alloue le frame heap, y stocke les arguments, état initial 0) et `<nom>__resume(frame:Ptr) -> Bool` (la machine à états : prologue de dispatch sur l'état sauvegardé — chaîne `CmpEq`+`Branch`, patron `class_dispatch.rs` — puis le corps normal, où `emit` devient `SetField(__value)`+`SetField(__state)`+`Return(true)`+nouveau bloc de reprise).
   - TOUS les paramètres et locales (dédupliqués par nom, même simplification que les `locals` déjà non scopés du lowering normal) sont promus dans le frame — aucune analyse de vivacité : plus simple, correct, un peu moins optimal (accepté pour cette première implémentation).
   - Nouveau champ `LowerBuilder::frame_vars` (accès direct GetField/SetField, sans la double-indirection `__locked_cell_*` des captures de closure — un générateur n'est jamais partagé entre threads) consulté en priorité par `declare_local`/`load_local`/`store_local`/`slot_of_local` : rend la transformation TRANSPARENTE pour tout le lowering de contrôle de flux existant (`if`/`while`/`switch`/`try`), aucune réécriture nécessaire là.
   - Frame alloué/libéré via `__alloc_obj`/`__free_obj` (nouveau, symétrique — préfixe `__` ⇒ pas de tag, même mécanisme que les env de closures).
   - Bug découvert et corrigé pendant l'implémentation : `expr_ir_type`/plusieurs boucles d'évaluation d'arguments (dont le dispatch typé `IO::write`/`IO::writeln`) lisaient `builder.locals` directement, invisible pour une variable de générateur (`frame_vars`) — corrigé à la source.
   - Bug d'ordonnancement découvert et corrigé : un générateur MÉTHODE consommé depuis une fonction lowered avant lui (ex: `main`) n'était pas trouvé (`module.message_funcs` pas encore peuplé) — un nouveau pré-passage (`register_all_message_funcs`) calcule le layout du frame et enregistre TOUS les générateurs du programme avant tout lowering de corps, même contrainte d'ordre que `class_dispatch::compute_classes_with_subclasses`.
4. **✅ Lowering — 2 des 3 formes de consommation** (`Array::fromMessage` reste l'Étape 6) :
   - `for x in truc() { ... }` : AUCUNE restriction (même avec `emit` en boucle). Frame libéré au point de fusion (`merge_bb`), atteint aussi bien par épuisement naturel que par `break` (même bloc cible) — un `return`/`raise` anticipé depuis le corps saute directement hors de la fonction sans jamais atteindre `merge_bb` et **fuit donc le frame** (fuite, jamais de corruption — même famille que les limitations déjà acceptées ailleurs dans ce projet). Non traité pour cette étape.
   - Consommation scalaire directe (`var x:T = truc()`, `return truc()`, argument d'un appel de fonction libre/statique — dont `IO::writeln(truc())`) : un seul `resume`, lecture de `__value`, libération immédiate du frame.
   - Vérifié bout en bout, PROGRAMMES RÉELS COMPILÉS ET EXÉCUTÉS (pas seulement `--check`) : fonction libre (emit simple, branchement `if`/`else`, boucle `while`), méthode d'instance, scalaire direct en argument, `for` avec `break` anticipé — tous corrects. `make regression` intégral repassé au vert (518+49 PASS, 0 FAIL) après chaque correctif.
5. **✅ Lowering — Cas A (`emit` dans un `try`)** (`crate::lower::builder::message_gen::lower_try_in_generator`) :
   - `try`/`on` DANS un générateur est lowered INLINE dans `__resume` (jamais en fonctions séparées `__try_body_N`/`__try_handler_N`) : `__ocara_try_enter` (push un `TryFrame`, nouveau) + `setjmp` (appelé DIRECTEMENT depuis le code généré — nouvelle entrée `BuiltinDesc` dans `src/codegen/desc.d/lowlevel.rs`, résolue au lien contre la libc) + branchement (0 = corps normal, non-nul = handler).
   - Un niveau d'imbrication = un champ frame `__try_frame_N` (des `try` FRÈRES au même niveau partagent le champ, comme tout autre local dédupliqué). Le handler est lowered UNE SEULE FOIS ; premier passage ET chaque rejeu y sautent tous via ce même bloc.
   - À chaque `emit` À L'INTÉRIEUR d'un `try` encore actif : `__ocara_try_exit` (nouveau) appelé une fois par niveau actif AVANT le `Return` (garde `TRY_STACK` équilibré à travers l'appel natif) ; le prologue de dispatch de `__resume` rejoue `__ocara_try_enter`+`setjmp` (chaîne imbriquée, un bloc "sas de reprise" par état) avant de sauter au bloc de reprise réel.
   - Le mécanisme `raise`/`__ocara_fail`/`TRY_STACK` existant n'est PAS modifié : un `raise` textuel dans le générateur ET un `raise` déclenché par une fonction APPELÉE depuis le `try` sont tous deux rattrapés (même pile partagée) ; l'imbrication respecte la sémantique LIFO normale ; un filtre de classe qui ne correspond à rien se re-propage vers un `try` extérieur (générateur ou non) exactement comme avant.
   - Vérifié bout en bout (programmes réels compilés et exécutés) : chemin normal sans exception, `raise` entre deux `emit` rattrapé par le handler du générateur (qui `emit` lui-même), `raise` depuis une fonction appelée, `try` imbriqués (2 niveaux, routage LIFO correct), filtre de classe non-correspondant repropagé vers un `try` extérieur au générateur. 24 assertions dédiées (`examples/tests/42_emit_generatorTest.oc`) + `make regression` intégral (542 + 49 PASS, 0 FAIL, exécuté après chaque étape de l'implémentation, comme demandé).
6. **✅ Runtime** : `Array::fromMessage(message<T>) -> array<T>` — traité entièrement à part (sema : `Expr::StaticCall` spécial-casé dans `typecheck.rs`, jamais un `FuncSig` normal, son type de retour dépend dynamiquement de l'argument ; lowering : `message_gen::lower_array_from_message`, drainage complet sans restriction, stockage brut comme un littéral `array<T>` concret — voir `LiteralElemKind::Concrete`). Vérifié par 10 assertions dédiées (fonction en boucle + générateur avec `try`/Cas A) + `make regression` (552 + 49 PASS, 0 FAIL). Bug PRÉ-EXISTANT et sans rapport découvert au passage (`Array::get` affiché `null` pour la valeur `0` en position d'argument direct) — documenté séparément, voir docs/roadmap.d/langage-array-get-display-bug.md.
7. **✅ Documentation** : `docs/EBNF.md` (nouvelle §28 "Générateurs (`emit`)", `message<T>` ajouté à la grammaire `Type`/`Statement`, table des matières et sections 29-31 renumérotées), `docs/workflow-compilation.md` (nouvelle sous-section dans la phase Lowering), `docs/adding-types.md` (nouvelle section "Cas particulier : un type RESTREINT" utilisant `message<T>` comme exemple de référence pour un futur type volontairement contraint).
8. **✅ Tests de régression** : `examples/tests/42_emit_generatorTest.oc` (34 assertions : `for`/scalaire pour fonctions ET méthodes, branchement, boucle, `break` anticipé, Cas A sous 5 formes, `Array::fromMessage` sous 2 formes).

## Fichiers clés

`src/parsing/` (lexer/parser), `src/parsing/ast.d/statements.rs` (`Stmt::Emit`), `src/parsing/ast.d/types.rs` (`Type::Message`), `src/sema/typecheck.rs`/`src/sema/message_emit.rs` (typage, diagnostics E30-E34, "au plus un emit hors boucle"), `src/lower/builder.d/message_gen.rs` (machine à états, `__new`/`__resume`, `frame_vars`, les 2 formes de consommation câblées, Cas A), `src/lower/builder.d/program.rs`/`classes.rs` (redirection vers `message_gen` + pré-passage `register_all_message_funcs`), `runtime/src/lib.rs` (`__free_obj`, `__ocara_try_enter`/`__ocara_try_exit`), `src/codegen/desc.d/lowlevel.rs` (`setjmp` appelable directement), `src/builtins/array.rs`/`runtime/src/lib.rs` (`Array::fromMessage`, pas encore fait), `examples/tests/42_emit_generatorTest.oc`, `docs/EBNF.md`, `docs/workflow-compilation.md`, `docs/adding-types.md`.
