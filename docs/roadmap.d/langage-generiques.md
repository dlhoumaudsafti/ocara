# Génériques : vérification incomplète et typage partiel

## ✅ Vérification d'arité — corrigée

`use List<int,string,Foo>()` sur un `generic List<T>` (1 seul paramètre attendu) ne produisait aucune erreur (2 `TODO` explicites : `src/sema/typecheck.rs:1109-1110,1122`). **Corrigé** : le nombre d'arguments de type est maintenant vérifié contre le nombre de paramètres déclarés par la `generic` (borne basse = paramètres sans valeur par défaut, borne haute = total des paramètres) — nouveau diagnostic **E21** (`SemaError::GenericArityMismatch`, `docs/diagnostics.md`). Remplace au passage l'ancien message trompeur ("is not a class") pour le cas `use Foo()` sans aucun argument de type.

Il n'existe pas de syntaxe de **contraintes** sur un paramètre de type (`T: SomeInterface`) dans le langage aujourd'hui — seulement des valeurs par défaut (`U=default`, voir `TypeParam.default`). Le TODO d'origine qui mentionnait "valider les contraintes (extends, implements)" faisait donc référence à l'arité, pas à une fonctionnalité de contraintes qui n'existe pas encore ; rien à vérifier de ce côté tant que cette syntaxe n'existe pas.

Vérifié manuellement (3 scénarios : arité correcte, trop d'arguments, aucun argument, plus un cas avec paramètre par défaut) ; `make regression` ne montre aucune régression.

## Ce qui manque encore : typage des valeurs génériques

- **Le typage des accès membres sur une valeur générique n'est pas vérifié** : `type_class_name` (src/sema/typecheck.rs:1277-1290) ne traite pas `Type::Generic` → `numbers.add("texte")` sur un `List<int>` ne lève aucune erreur, retombe silencieusement sur `Type::Mixed`.
- **Un générique ne fonctionne qu'en variable locale directement initialisée** (`var x:List<int> = use List<int>()`). Passé en paramètre de fonction, champ de classe ou valeur de retour, la résolution de classe casse (`src/lower/builder.d/functions.rs:119-122` n'enregistre `var_class` que pour `Type::Named`, jamais `Type::Generic`).
- `implements` sur un `generic` n'est jamais vérifié (la boucle E09 itère sur `program.classes` avant que la monomorphisation ne produise les classes concrètes).
- `substitute_type` ne renomme jamais `Type::Generic` en `Type::Named` après monomorphisation (monomorph.rs:69-76) — la cohérence ne tient que parce que le lowering recalcule indépendamment le même nom mangé à 2 endroits distincts (`src/lower/stmt.d/statements.d/variables.rs:38-42` et `:126-130`), une duplication fragile plutôt qu'une source unique de vérité.

## Ampleur

Plusieurs jours à quelques semaines : étendre le typecheck à `Type::Generic`, unifier la substitution de type, propager `var_class` aux paramètres/champs/retours, décider du traitement des fonctions génériques. Aucun exemple ni test du dépôt n'utilise aujourd'hui un générique en paramètre/champ/retour, et `examples/generics/` ne compile même plus en l'état (voir [langage-syntaxe-obsolete](langage-syntaxe-obsolete.md)).

## Fichiers clés

`src/core/monomorph.rs`, `src/sema/typecheck.rs`, `src/sema/error.rs`, `src/lower/builder.d/functions.rs`, `src/lower/stmt.d/statements.d/variables.rs`.
