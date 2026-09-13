# `try`/`on`/`raise` : vérifications sémantiques manquantes

## Constat

Pas de `Result<T,E>` builtin — `Result` (docs/EBNF.md) est une classe générique pédagogique construite par l'utilisateur au-dessus de `raise`, qui reste la seule primitive réelle du langage. Le parsing/AST est fidèle à l'EBNF et le lowering/runtime (modèle "callback" + `setjmp`/`longjmp`) est fonctionnel et testé.

Manques constatés :

- **Aucune vérification du nom de classe dans `on e is X`** — un typo (`on e is Typo`) compile silencieusement, le handler devient mort sans le moindre diagnostic.
- **La règle "le catch-all doit être en dernier" n'est pas imposée** par le compilateur, alors que documentée (EBNF §28.2).
- **Aucune hiérarchie d'exceptions réelle** : `__ocara_type_matches` (runtime/src/lib.rs:2540-2548) est une égalité de chaînes stricte, sans parcours de `extends` — une sous-classe d'une classe filtrée ne serait pas attrapée par un filtre sur la classe parente (non documenté). Les ~19 exceptions builtin sont toutes générées par la même fonction avec les 3 mêmes champs — une struct dupliquée sous des noms différents, pas une vraie hiérarchie de classes.

## Ampleur

Chaque vérification sémantique manquante est un ajout circonscrit à `sema` (vérifier le nom de classe existant, imposer l'ordre du catch-all). La hiérarchie d'exceptions réelle est plus structurelle : elle demanderait de faire marcher `extends` dans le filtrage runtime, ce qui touche le modèle actuel des exceptions de bout en bout.

## Fichiers clés

`src/sema/` (absence de `class_filter`), `runtime/src/lib.rs` (`__ocara_type_matches`), `src/builtins/exception.rs` (`make_exception_class`).
