# Génériques : vérification incomplète et typage partiel

## ✅ Vérification d'arité — corrigée précédemment

`use List<int,string,Foo>()` sur un `generic List<T>` (1 seul paramètre attendu) ne produisait aucune erreur (2 `TODO` explicites : `src/sema/typecheck.rs:1109-1110,1122`). **Corrigé** : le nombre d'arguments de type est vérifié contre le nombre de paramètres déclarés par la `generic` — diagnostic **E21**.

## ✅ Typage des accès membres sur une valeur générique — corrigé

`type_class_name`/le dispatch d'appel de méthode ne traitaient pas `Type::Generic` → `numbers.add("texte")` sur un `List<int>` ne levait aucune erreur, retombait silencieusement sur `Type::Mixed`. **Corrigé** (`src/sema/typecheck.rs`) :
- Nouveau cas dédié dans le typecheck d'un appel de méthode (`Expr::Field` en callee) : quand le récepteur est `Type::Generic { name, args }`, la méthode est cherchée dans la déclaration `generic` (`SymbolTable::lookup_generic`), avec une vraie **substitution des paramètres de type** par les arguments concrets de cette instance (nouvelle fonction `substitute_type_params` — `T` → `int` pour `List<int>`, y compris récursivement dans `array<T>`, `map<K,T>`, unions, etc.).
- Arité, type de chaque argument (substitué) et type de retour (substitué) sont maintenant vérifiés — `boxInt.set("texte")` sur un `Box<int>` est rejeté (`expected type 'int', found 'string'`), un usage correct type-check et s'exécute normalement.
- Méthode inconnue sur le générique → diagnostic `FieldNotFound` (au lieu du silence total précédent).

Vérifié manuellement (méthode invalide rejetée, méthode valide acceptée avec substitution correcte du type de retour) ; `make regression` sans régression.

**Découverte annexe, non corrigée** : en testant une valeur retournée par une méthode générique puis réassignée à un champ (`self.value = v` dans `Box<T>::set`), le programme compilé produit un résultat runtime incorrect (`b.set(100); IO::writeln(b.get())` affiche `1` au lieu de `100`) — confirmé **pré-existant** (identique avant ce correctif, via `git stash`). Cette découverte est cohérente avec le sous-point ci-dessous ("un générique ne fonctionne qu'en variable locale directement initialisée") : le lowering des champs/méthodes d'une classe monomorphisée a manifestement d'autres lacunes que la seule vérification de type côté sema corrigée ici.

## Ce qui manque encore (non traité — chantier de plusieurs jours, cf. ampleur)

- **Un générique ne fonctionne correctement qu'en variable locale directement initialisée** (`var x:List<int> = use List<int>()`). Passé en paramètre de fonction, champ de classe ou valeur de retour, la résolution de classe casse (`src/lower/builder.d/functions.rs:119-122` n'enregistre `var_class` que pour `Type::Named`, jamais `Type::Generic`) — et la découverte ci-dessus suggère que même l'usage "simple" (champ d'instance dans une méthode du générique lui-même) a des lacunes de lowering non cataloguées précédemment.
- `implements` sur un `generic` n'est jamais vérifié (la boucle E09, dans `src/main.rs`, itère sur `program.classes` avant que la monomorphisation ne produise les classes concrètes — `program.generics` n'est jamais parcouru par cette vérification).
- `substitute_type` (`src/core/monomorph.rs:69-76`) ne renomme jamais `Type::Generic` en `Type::Named` après monomorphisation — la cohérence ne tient que parce que le lowering recalcule indépendamment le même nom mangé à 2 endroits distincts (`src/lower/stmt.d/statements.d/variables.rs:38-42` et `:126-130`), une duplication fragile plutôt qu'une source unique de vérité.

## Ampleur (restante)

Plusieurs jours à quelques semaines : propager `var_class`/la substitution de type aux paramètres/champs/valeurs de retour dans TOUT le pipeline de lowering (pas seulement au niveau sema, corrigé ici), déboguer la cause exacte du bug de champ découvert ci-dessus, étendre l'E09 aux génériques, unifier la substitution de nom mangé. Aucun exemple ni test du dépôt n'utilise aujourd'hui un générique en paramètre/champ/retour, et `examples/generics/` reste hors CI (voir [langage-syntaxe-obsolete](langage-syntaxe-obsolete.md)/[qualite-couverture-tests](qualite-couverture-tests.md)) — peu de filet de sécurité pour un chantier de cette taille.

## Fichiers clés

`src/sema/typecheck.rs` (`substitute_type_params`, nouveau cas `Type::Generic` dans le typecheck d'appel de méthode), `src/core/monomorph.rs`, `src/lower/builder.d/functions.rs`, `src/lower/stmt.d/statements.d/variables.rs`, `src/main.rs` (boucle E09).
