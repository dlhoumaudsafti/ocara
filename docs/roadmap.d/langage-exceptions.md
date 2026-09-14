# `try`/`on`/`raise` : vérifications sémantiques manquantes

## ✅ Corrigé — validation du nom de classe et ordre du catch-all

Pas de `Result<T,E>` builtin — `Result` (docs/EBNF.md) est une classe générique pédagogique construite par l'utilisateur au-dessus de `raise`, qui reste la seule primitive réelle du langage. Le parsing/AST est fidèle à l'EBNF et le lowering/runtime (modèle "callback" + `setjmp`/`longjmp`) est fonctionnel et testé.

Deux manques corrigés (`src/sema/typecheck.rs`, `Stmt::Try`) :

- **Aucune vérification du nom de classe dans `on e is X`** — un typo (`on e is Typo`) compilait silencieusement, le handler devenait mort sans le moindre diagnostic. **Corrigé** : nouveau diagnostic **E23**, `X` doit correspondre à une classe connue (`SymbolTable::lookup_class`).
- **La règle "le catch-all doit être en dernier" n'était pas imposée**, alors que documentée (EBNF §28.2). **Corrigé** : nouveau diagnostic **E24**.

**Découverte annexe corrigée en cours de route** : `SDLException`/`TauriException` sont réellement levées par le runtime (`runtime/src/exception.rs::throw_sdl_exception`/`throw_tauri_exception`) mais n'étaient enregistrées **nulle part** côté sema (`src/builtins/mod.rs::all_builtins`/`builtin_class`, ni dans `OCARA_BUILTINS` de `src/main.rs`) — `on e is SDLException` (déjà utilisé tel quel dans `examples/builtins/sdl.oc`, sans jamais importer explicitement `ocara.SDLException`) aurait été rejeté à tort par le nouveau E23 sans ce correctif. Ajoutées comme les ~19 autres classes d'exception builtin.

Pour que E23 n'exige jamais un import explicite d'une classe d'exception (confirmé comme le pattern réel du dépôt : `on e is SDLException` après un simple `import ocara.SDL`, jamais `import ocara.SDLException`), toutes les classes d'exception builtin sont désormais enregistrées automatiquement dans `SymbolTable::new()` (comme `String`/`Array`/`Map`/`JSON` l'étaient déjà) — "ambiantes", pas besoin d'import.

Vérifié : `on e is TypoException` → rejeté (E23) ; un handler catch-all suivi d'un autre handler → rejeté (E24) ; les usages réels existants (`on e is SDLException`/`FileException`/`SQLiteException`/... sans import explicite de l'exception) continuent de compiler. Deux nouveaux cas ajoutés à `examples/21_errors.oc` (fichier "doit échouer à la compilation", déjà suivi par `ci/regression.sh`). `make regression` sans régression (386 PASS / 0 FAIL, 49 PASS / 0 FAIL / 0 ERREUR côté projet).

## ✅ Corrigé — vraie hiérarchie d'exceptions (`extends` reconnu par `on e is X`)

**Symptôme** : `__ocara_type_matches` (`runtime/src/lib.rs`) était une égalité de chaînes stricte, sans parcours de `extends` — un filtre sur une classe parente n'attrapait pas une sous-classe (`on e is Parent` n'attrapait pas `Enfant extends Parent`). Les ~19 exceptions builtin, toutes générées par `make_exception_class` avec `extends: None`, n'avaient elles-mêmes aucune hiérarchie entre elles.

**Corrigé**, en deux moitiés symétriques (une par origine possible d'un `raise`) :

- **Côté code Ocara compilé** (`raise` dans `lower_raise`, `src/lower/stmt.d/statements.d/exceptions.rs`) : le nom de classe statiquement connu (littéral `use Classe(...)` **ou**, nouveau, une variable dont la classe est connue via `var_class` — `raise err` fonctionne désormais aussi, pas seulement `raise use Classe(...)` inline) n'est plus passé seul à `__ocara_fail` : `IrModule::ancestor_chain` (nouvelle méthode, `src/ir/module.rs`) calcule sa chaîne d'ancêtres complète en remontant `class_parents`, encodée comme une chaîne jointe par `|` (ex. `"Enfant|Parent"`).
- **Côté exceptions levées directement par le runtime** (`File::read` sur un fichier inexistant, etc.) : chaque `throw_*_exception` (`runtime/src/exception.rs`) passait juste son propre nom ; chacune passe désormais `"<Nom>|Exception"` (toutes les exceptions builtin sont des sous-classes directes d'`Exception`, hiérarchie plate d'un niveau, câblée en dur plutôt que via un mécanisme générique — inutile ici, cette hiérarchie ne varie jamais).
- `__ocara_type_matches` cherche maintenant le filtre comme un des maillons de la chaîne (`stored.split('|').any(...)`) au lieu d'une égalité stricte — reste compatible avec un `stored` à un seul maillon.
- `class_parents` (déjà utilisé par le codegen pour la résolution de méthodes héritées) est maintenant aussi peuplé pour les ~19 exceptions builtin (`Nom → "Exception"`, voir `src/lower/builder.d/program.rs`), en plus des classes utilisateur (`extends`, déjà couvert).
- Le message d'erreur "UNHANDLED EXCEPTION" (`__ocara_fail`, `runtime/src/lib.rs`) n'affiche que le premier maillon de la chaîne (le nom réellement levé), pas la chaîne entière.

**Reste, comme avant, seulement attrapable par un handler générique (`on e` sans `is`)** : un `raise` d'une expression dont le type n'est pas connu statiquement (`mixed`, valeur calculée dynamiquement, chaîne littérale...) — cas déjà documenté, inchangé.

Vérifié : hiérarchie utilisateur pure (`Enfant extends Parent`), chaîne à 3 niveaux, sous-classe utilisateur d'une exception builtin (`class MonErreur extends FileException`), `raise` d'une variable, exception builtin réellement levée par le runtime attrapée via `Exception`, contrôle négatif (filtre non lié ne capture pas), et le nom exact de la sous-classe continue de fonctionner — `examples/tests/33_exception_hierarchyTest.oc` (10 assertions). `make regression` sans régression.

## Fichiers clés

`src/sema/typecheck.rs` (`Stmt::Try`, E23/E24), `src/sema/symbols.rs` (`SymbolTable::new`, enregistrement automatique des exceptions), `src/sema/error.rs` (`OnFilterClassNotFound`, `CatchAllNotLast`), `src/builtins/mod.rs`/`src/builtins/exception.rs` (`SDLException`/`TauriException` ajoutées, `BUILTIN_EXCEPTION_NAMES`), `src/ir/module.rs` (`IrModule::ancestor_chain`), `src/lower/builder.d/program.rs` (peuplement de `class_parents` pour les exceptions builtin), `src/lower/stmt.d/statements.d/exceptions.rs` (`lower_raise`), `runtime/src/lib.rs` (`__ocara_type_matches`, `__ocara_fail`), `runtime/src/exception.rs` (`throw_*_exception`), `examples/tests/33_exception_hierarchyTest.oc`.
