# Génériques : vérification incomplète et typage partiel

## Ce qui existe

Une vraie monomorphisation AST→AST (src/core/monomorph.rs, 575 lignes, appelée une fois dans src/main.rs:529 après le typecheck, avant le lowering) : pour chaque combinaison de types rencontrée via `use Foo<T>()`, une classe spécialisée réelle est générée (ex. `List_int`). Ce n'est pas du type erasure.

## Ce qui manque

- **Aucune vérification d'arité ni de contraintes** des paramètres de type (2 `TODO` explicites non traités : src/sema/typecheck.rs:1109-1110,1122). `use List<int,string,Foo>()` sur un `generic List<T>` (1 seul paramètre attendu) ne produit aucune erreur.
- **Le typage des accès membres sur une valeur générique n'est pas vérifié** : `type_class_name` (src/sema/typecheck.rs:1277-1290) ne traite pas `Type::Generic` → `numbers.add("texte")` sur un `List<int>` ne lève aucune erreur, retombe silencieusement sur `Type::Mixed`.
- **Un générique ne fonctionne qu'en variable locale directement initialisée** (`var x:List<int> = use List<int>()`). Passé en paramètre de fonction, champ de classe ou valeur de retour, la résolution de classe casse (src/lower/builder.d/functions.rs:119-122 n'enregistre `var_class` que pour `Type::Named`, jamais `Type::Generic`).
- `implements` sur un `generic` n'est jamais vérifié (la boucle E09 itère sur `program.classes` avant que la monomorphisation ne produise les classes concrètes).
- `substitute_type` ne renomme jamais `Type::Generic` en `Type::Named` après monomorphisation (monomorph.rs:69-76) — la cohérence ne tient que parce que le lowering recalcule indépendamment le même nom mangé à 2 endroits distincts (src/lower/stmt.d/statements.d/variables.rs:38-42 et :126-130), une duplication fragile plutôt qu'une source unique de vérité.

## Ampleur

Plusieurs jours à quelques semaines : étendre le typecheck à `Type::Generic`, unifier la substitution de type, propager `var_class` aux paramètres/champs/retours, décider du traitement des fonctions génériques. Aucun exemple ni test du dépôt n'utilise aujourd'hui un générique en paramètre/champ/retour, et `examples/generics/` ne compile même plus en l'état (voir [langage-syntaxe-obsolete](langage-syntaxe-obsolete.md)).

## Fichiers clés

`src/core/monomorph.rs`, `src/sema/typecheck.rs`, `src/lower/builder.d/functions.rs`, `src/lower/stmt.d/statements.d/variables.rs`.
