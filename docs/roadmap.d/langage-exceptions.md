# `try`/`on`/`raise` : vérifications sémantiques manquantes

## ✅ Corrigé — validation du nom de classe et ordre du catch-all

Pas de `Result<T,E>` builtin — `Result` (docs/EBNF.md) est une classe générique pédagogique construite par l'utilisateur au-dessus de `raise`, qui reste la seule primitive réelle du langage. Le parsing/AST est fidèle à l'EBNF et le lowering/runtime (modèle "callback" + `setjmp`/`longjmp`) est fonctionnel et testé.

Deux manques corrigés (`src/sema/typecheck.rs`, `Stmt::Try`) :

- **Aucune vérification du nom de classe dans `on e is X`** — un typo (`on e is Typo`) compilait silencieusement, le handler devenait mort sans le moindre diagnostic. **Corrigé** : nouveau diagnostic **E23**, `X` doit correspondre à une classe connue (`SymbolTable::lookup_class`).
- **La règle "le catch-all doit être en dernier" n'était pas imposée**, alors que documentée (EBNF §28.2). **Corrigé** : nouveau diagnostic **E24**.

**Découverte annexe corrigée en cours de route** : `SDLException`/`TauriException` sont réellement levées par le runtime (`runtime/src/exception.rs::throw_sdl_exception`/`throw_tauri_exception`) mais n'étaient enregistrées **nulle part** côté sema (`src/builtins/mod.rs::all_builtins`/`builtin_class`, ni dans `OCARA_BUILTINS` de `src/main.rs`) — `on e is SDLException` (déjà utilisé tel quel dans `examples/builtins/sdl.oc`, sans jamais importer explicitement `ocara.SDLException`) aurait été rejeté à tort par le nouveau E23 sans ce correctif. Ajoutées comme les ~19 autres classes d'exception builtin.

Pour que E23 n'exige jamais un import explicite d'une classe d'exception (confirmé comme le pattern réel du dépôt : `on e is SDLException` après un simple `import ocara.SDL`, jamais `import ocara.SDLException`), toutes les classes d'exception builtin sont désormais enregistrées automatiquement dans `SymbolTable::new()` (comme `String`/`Array`/`Map`/`JSON` l'étaient déjà) — "ambiantes", pas besoin d'import.

Vérifié : `on e is TypoException` → rejeté (E23) ; un handler catch-all suivi d'un autre handler → rejeté (E24) ; les usages réels existants (`on e is SDLException`/`FileException`/`SQLiteException`/... sans import explicite de l'exception) continuent de compiler. Deux nouveaux cas ajoutés à `examples/21_errors.oc` (fichier "doit échouer à la compilation", déjà suivi par `ci/regression.sh`). `make regression` sans régression (386 PASS / 0 FAIL, 49 PASS / 0 FAIL / 0 ERREUR côté projet).

## Non traité — hiérarchie d'exceptions réelle

**Aucune hiérarchie d'exceptions réelle** : `__ocara_type_matches` (`runtime/src/lib.rs`) reste une égalité de chaînes stricte, sans parcours de `extends` — une sous-classe d'une classe filtrée n'est pas attrapée par un filtre sur la classe parente (documenté maintenant dans `docs/EBNF.md` §28.2). Les ~19 exceptions builtin sont toutes générées par la même fonction (`make_exception_class`) avec les 3 mêmes champs et `extends: None` — une struct dupliquée sous des noms différents, pas une vraie hiérarchie de classes.

**Pourquoi non traité ici** : contrairement aux deux corrections ci-dessus (ajouts circonscrits à `sema`), faire marcher `extends` dans le filtrage runtime toucherait le modèle actuel des exceptions de bout en bout — il faudrait au minimum (1) déclarer une hiérarchie explicite entre les classes d'exception builtin (aujourd'hui toutes plates, `extends: None`), et (2) faire porter au runtime, pour chaque objet levé, de quoi retrouver sa chaîne d'ancêtres (`__ocara_type_matches` ne reçoit aujourd'hui que deux noms de type bruts, pas de chaîne de classes) — un changement de représentation qui dépasse le seul ajout d'une vérification.

## Fichiers clés

`src/sema/typecheck.rs` (`Stmt::Try`, E23/E24), `src/sema/symbols.rs` (`SymbolTable::new`, enregistrement automatique des exceptions), `src/sema/error.rs` (`OnFilterClassNotFound`, `CatchAllNotLast`), `src/builtins/mod.rs`/`src/builtins/exception.rs` (`SDLException`/`TauriException` ajoutées), `src/main.rs` (`OCARA_BUILTINS`), `runtime/src/lib.rs` (`__ocara_type_matches`, non touché), `src/builtins/exception.rs` (`make_exception_class`, non touché).
