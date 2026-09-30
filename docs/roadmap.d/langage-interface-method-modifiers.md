# Modificateurs incomplets sur une méthode d'`interface` — seules 2 combinaisons sur 12 acceptées

Vérifié :
- **Les 12 combinaisons parsent désormais, plus la forme historique sans le
  moindre modificateur** (13 formes au total, chacune vérifiée séparément,
  pas juste "ça compile") — `src/parsing/parser.d/declarations.rs::parse_interface_method`
  accepte maintenant `[public|private|protected]? static? async? method`,
  en symétrie avec `parse_class_member`/`parse_method_decl` pour une
  `class`. La visibilité reste **optionnelle** (contrairement à
  `parse_visibility` pour une classe, où elle est obligatoire) : la forme
  historique `method nom(): T` sans aucun modificateur, valide avant même
  `wiring`, n'a jamais été cassée.
- **`InterfaceMethod` porte désormais `is_async`** (`src/parsing/ast.d/interfaces.rs`),
  en miroir de `is_static` déjà présent, et `register_interface` le
  propage dans `FuncSig` (`src/sema/symbols.d/registers.rs`).
- **Décision sur la visibilité (la seule nuance ouverte du ticket)** :
  vérifié que `FuncSig` (table des symboles) ne porte AUCUN champ de
  visibilité pour QUELQUE méthode que ce soit dans ce compilateur — ni pour
  une classe ordinaire, ni pour une vérification `extends`. La visibilité
  n'a donc jamais fait partie d'une comparaison de signature nulle part
  dans ce langage ; imposer une vérification stricte de visibilité
  UNIQUEMENT pour `implements` aurait été une asymétrie NOUVELLE avec
  `class`, à l'opposé de l'objectif explicite de ce ticket (symétrie), et
  sans le moindre cas d'usage concret sans corps de méthode par défaut dans
  ce langage. Décision : le token de visibilité est accepté puis jeté
  (comme `public` l'était déjà avant ce ticket), jamais stocké ni vérifié —
  documenté explicitement dans `InterfaceMethod`, `docs/EBNF.md` §17 et ce
  ticket.
- **`static`/`is_async`, eux, ONT une sémantique directe et sont désormais
  vérifiés à la conformité `implements`** — extension de la vérification
  E09 déjà existante pour `is_static` (`src/main.rs`, boucles classe ET
  generic) : une interface qui exige `async method` doit être honorée par
  une méthode `async`, réciproquement pour une méthode non-`async`.
- **Bug préexistant trouvé et corrigé en implémentant cette vérification**
  (`src/sema/symbols.d/registers.rs`) : `register_class`/`register_module`/
  `register_generic` codaient TOUS EN DUR `is_async: false` pour une
  méthode, quelle que soit sa déclaration réelle (`register_function`, pour
  les fonctions LIBRES, le fait correctement depuis toujours). Sans
  conséquence observable jusqu'ici — rien ne consultait `FuncSig.is_async`
  pour une méthode, le codegen `async` de classe utilisant un mécanisme
  entièrement différent basé sur l'AST (`src/lower/builder.d/program.rs`,
  `async_funcs`) — mais aurait rendu ma nouvelle vérification de conformité
  TOUJOURS en échec dès qu'une interface exige `async` (confirmé par
  reproduction avant correctif : une classe pourtant conforme,
  `public async method`, était rejetée comme non-conforme). Corrigé en
  propageant `fd.is_async` au lieu de `false`, pour les 3 sites.
- **Deuxième bug préexistant découvert EN COURS DE ROUTE, sans rapport avec
  ce ticket, en essayant d'écrire le test de régression bout-en-bout pour
  `async`** : appeler une méthode D'INSTANCE `async` via le sucre
  `obj.methode()` SIGSEGV, pour N'IMPORTE QUELLE classe — confirmé isolé
  d'abord avec une interface, puis reproduit avec une classe CONCRÈTE
  ordinaire sans la moindre interface en jeu. Root cause localisée
  précisément par gdb (crash dans `__task_resolve`, appelé avec une valeur
  qui n'est pas un vrai task handle) et par `--dump` HIR (le site d'appel
  émet un appel DIRECT vers la méthode synchrone, jamais vers son wrapper
  `__async_wrap_.../__task_spawn`) : `src/lower/expr.d/lower.rs`, le bras
  `Expr::Call { callee: Expr::Field }` (sucre d'instance) ne consulte
  JAMAIS `builder.async_funcs`, contrairement aux DEUX autres formes
  d'appel (fonction libre, `Classe::methode()` statique) qui le font déjà
  correctement — confirmé que `static async method`, appelée via
  `Classe::methode()`, fonctionne parfaitement. Délibérément NON corrigé
  ici (hors périmètre — ticket de grammaire, pas de codegen async ;
  correctif estimé Structurel, pas une simple vérification manquante,
  voir sa propre fiche) : documenté séparément dans
  [langage-async-instance-method-dispatch-broken](langage-async-instance-method-dispatch-broken.md),
  ajouté à `docs/roadmap.md` en Priorité Haute. Le test de régression de ce
  ticket-ci exerce `async` UNIQUEMENT via `static async` (le seul chemin
  fonctionnel aujourd'hui), documenté explicitement comme un contournement
  délibéré, pas un oubli de couverture.
- **7 nouveaux tests Rust** : `src/parsing/parser.d/tests.rs` (1 test
  paramétré couvrant les 12 combinaisons + la forme bare, vérifiant
  `is_static`/`is_async` exacts pour chacune) ; `src/sema/tests/interface_method_modifiers.rs`
  (nouveau fichier, 6 tests : propagation `is_async` correcte pour
  interface/classe/generic/module, non-régression sur une méthode
  non-async). `cargo test --bin ocara --release` : **175 passed, 0 failed**
  (168 avant ce ticket + 7).
- **Exemple de régression bout-en-bout** : `examples/62_interface_method_modifiers.oc`
  (illustratif, une interface avec les 5 mots-clés répartis sur 5 méthodes,
  implémentée et appelée) et `examples/tests/62_interface_method_modifiersTest.oc`
  (10 assertions : chacun des 5 modificateurs individuellement, plus un test
  global). `examples/advanced/mini_project`/`mini_project_hexa`
  explicitement PAS touchés.
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning.
  `./ci/regression.sh` : tous verts. `./ci/unittests.sh examples/project/tests` :
  50 PASS / 0 FAIL (inchangé). `./ci/unittests.sh examples/tests` :
  **804 PASS / 0 FAIL, 0 ERREUR(S)** (794 avant ce ticket, +10 nouvelles
  assertions).
- `docs/EBNF.md` : §17 (règle `InterfaceMethod`/nouvelle `InterfaceVisibility`,
  paragraphe expliquant la décision visibilité vs `static`/`async`) et §31
  (grammaire consolidée) mises à jour en miroir, §31 relue intégralement
  après coup (convention du projet). `docs/diagnostics.md` : pas de
  nouveau code E- introduit (la vérification `is_async` réutilise
  exactement le mécanisme/message E09 déjà existant pour `is_static`,
  comme demandé) — entrée E09 existante volontairement laissée telle
  quelle (déjà terse par convention, n'énumère pas tout ce qu'E09 vérifie
  — arité/types non plus).

## Constat (ticket original)

Une méthode de **classe** ordinaire acceptait déjà `public`/`private`/
`protected` combinés librement à `static` et `async` ; seules 2 des 12
combinaisons équivalentes passaient dans le corps d'une `interface`
(`public method`, `public static method` — ajoutées comme prérequis du
chantier `wiring`, jamais pensées comme une grammaire complète). Demande
explicite de l'utilisateur : symétrie complète avec `class`.

## Priorité / Complexité

**Terminé.** Complexité confirmée Légère pour la grammaire elle-même (la
symétrie avec `class` était bien mécanique, comme anticipé) — mais DEUX
bugs préexistants et sans rapport direct ont été trouvés en implémentant
la vérification de conformité `is_async` (voir ci-dessus), l'un corrigé
dans ce même chantier (propagation `is_async` dans `FuncSig` pour les
méthodes de classe/module/generic — un vrai trou de données, jamais
consommé jusqu'ici donc jamais dangereux, mais faux), l'autre documenté et
reporté (dispatch d'instance `async` cassé en général, Structurel, sa
propre fiche).

## Fichiers clés

`src/parsing/ast.d/interfaces.rs` (`InterfaceMethod::is_async`),
`src/parsing/parser.d/declarations.rs` (`parse_interface_method`),
`src/sema/symbols.d/registers.rs` (`register_interface` + correctif
`is_async` pour `register_class`/`register_module`/`register_generic`),
`src/main.rs` (vérification de conformité `is_async`, boucles 4d/4d-bis),
`src/parsing/parser.d/tests.rs`, `src/sema/tests/interface_method_modifiers.rs`
(+ `src/sema/tests/mod.rs`), `docs/EBNF.md` (§17, §31),
`examples/62_interface_method_modifiers.oc`,
`examples/tests/62_interface_method_modifiersTest.oc`.
