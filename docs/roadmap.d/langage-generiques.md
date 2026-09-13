# Génériques : vérification incomplète et typage partiel

## ✅ Vérification d'arité — corrigée précédemment

`use List<int,string,Foo>()` sur un `generic List<T>` (1 seul paramètre attendu) ne produisait aucune erreur (2 `TODO` explicites : `src/sema/typecheck.rs:1109-1110,1122`). **Corrigé** : le nombre d'arguments de type est vérifié contre le nombre de paramètres déclarés par la `generic` — diagnostic **E21**.

## ✅ Typage des accès membres sur une valeur générique — corrigé

`type_class_name`/le dispatch d'appel de méthode ne traitaient pas `Type::Generic` → `numbers.add("texte")` sur un `List<int>` ne levait aucune erreur, retombait silencieusement sur `Type::Mixed`. **Corrigé** (`src/sema/typecheck.rs`) :
- Nouveau cas dédié dans le typecheck d'un appel de méthode (`Expr::Field` en callee) : quand le récepteur est `Type::Generic { name, args }`, la méthode est cherchée dans la déclaration `generic` (`SymbolTable::lookup_generic`), avec une vraie **substitution des paramètres de type** par les arguments concrets de cette instance (nouvelle fonction `substitute_type_params` — `T` → `int` pour `List<int>`, y compris récursivement dans `array<T>`, `map<K,T>`, unions, etc.).
- Arité, type de chaque argument (substitué) et type de retour (substitué) sont maintenant vérifiés — `boxInt.set("texte")` sur un `Box<int>` est rejeté (`expected type 'int', found 'string'`), un usage correct type-check et s'exécute normalement.
- Méthode inconnue sur le générique → diagnostic `FieldNotFound` (au lieu du silence total précédent).

## ✅ Un générique fonctionne maintenant en paramètre, champ de classe et valeur de retour — corrigé

Seule une variable locale directement initialisée (`var x:List<int> = use List<int>()`) fonctionnait correctement. Confirmé par reproduction : passer une valeur générique en **paramètre** de fonction/méthode (`function useBox(b:Box<int>): int { return b.get() }` renvoyait `0` au lieu de la vraie valeur), ou y accéder via un **champ de classe** (`self.box.get()`/`c.box.get()` avec `property box:Box<int>` renvoyait `0`) appelait un symbole inexistant (`_method_get`) faute de savoir quelle classe monomorphisée dispatcher.

**Cause** : `var_class` (la table `nom de variable → classe monomorphisée`, consultée à chaque accès de champ/appel de méthode côté lowering) n'était alimentée pour un type `Type::Generic` que par la déclaration d'une variable locale (`lower_var`/`lower_const`, `src/lower/stmt.d/statements.d/variables.rs`) — jamais pour un paramètre de fonction/méthode (`src/lower/builder.d/functions.rs`), et `resolve_chained_field_class` (`src/lower/expr.d/helpers.rs`, résolution de la classe d'un champ chaîné `a.b.c`) ne savait reconnaître qu'un champ `Type::Named`/`String`/`Array`/`Map`, jamais `Type::Generic`.

**Corrigé** :
- `src/lower/builder.d/functions.rs` : un paramètre de type `Type::Generic { name, args }` est maintenant enregistré dans `var_class` avec son nom monomorphisé (`monomorphized_name`), exactement comme une variable locale.
- `src/lower/expr.d/helpers.rs` (`resolve_chained_field_class`) : un champ de type `Type::Generic` résout maintenant aussi vers son nom monomorphisé — couvre `self.champGénérique.méthode()` et `objet.champGénérique.méthode()`.
- `src/lower/builder.d/class_ownership.rs` (`classify_field`, génération de `__free_<Classe>`/`__clone_<Classe>`) : un champ de type `Type::Generic` est maintenant traité comme un objet possédé (libéré/cloné récursivement via le `__free_`/`__clone_` de sa classe monomorphisée) — avant ce correctif, un tel champ était silencieusement traité comme `Plain` : jamais libéré (fuite) et copié par pointeur brut (aliasing) au lieu d'être cloné en profondeur.

Vérifié par reproduction (paramètre, champ simple, champ générique auto-référent façon liste chaînée `Node<T> { property next:Node<T> }`, valeur de retour assignée à une variable/passée en argument, `scoped`/libération+clonage récursifs via `--dump`) ; `make regression` sans régression (386 PASS / 0 FAIL, seule l'erreur pré-existante déjà documentée `mainTest.oc`/`Printable` subsiste).

Cas explicitement testé et non affecté par ce correctif (limitation pré-existante et **distincte**, déjà cataloguée) : passer une `scoped`/`consumed` générique en argument d'une méthode qui la stocke dans un champ (`a.setNext(b)` où `b` est `scoped`) reste sujet au bug d'échappement par argument — voir [memoire-echappement-argument](memoire-echappement-argument.md). Pas une régression : le même risque existe à l'identique pour une classe non générique.

## ✅ `implements` sur un `generic` est maintenant vérifié — corrigé

La vérification E09 (`src/main.rs`) n'itérait que sur `program.classes`, or la monomorphisation (qui transforme chaque instanciation en classe concrète) ne tourne qu'**après** ce point — un `generic Bag<T> implements Sized` sans la méthode `size()` compilait sans la moindre erreur, quel que soit le nombre de fois où `Bag` était instancié. **Corrigé** : une boucle dédiée parcourt maintenant aussi `program.generics`, avec la même vérification (interface trouvée, méthode présente, arité/types de paramètres/type de retour compatibles) en s'appuyant sur `SymbolTable::lookup_generic` (déjà enregistré à cette étape). Vérifié : un générique incomplet est rejeté (`generic 'Bag' does not implement method 'size' from interface 'Sized'`), un générique correct compile et s'exécute.

## Non traité (évalué, jugé hors de propos)

`substitute_type` (`src/core/monomorph.rs:69-76`) ne renomme jamais `Type::Generic` en `Type::Named` après monomorphisation — la cohérence tient uniquement parce que le lowering recalcule indépendamment le même nom mangé (`monomorphized_name`, fonction pure) partout où c'est nécessaire, ce que confirment tous les tests ci-dessus (y compris le cas auto-référent `Node<T>`). Évalué et volontairement laissé tel quel :
- Aucun bug constaté n'en dépend — tous les points d'entrée qui ont besoin de connaître la classe d'une valeur générique (`var_class`, `resolve_chained_field_class`, `classify_field`) traitent maintenant `Type::Generic` directement, sans avoir besoin que l'AST ait été réécrit en amont.
- Un renommage effectif dégraderait la qualité des messages d'erreur : `type_name()` affiche déjà `Type::Generic` sous une forme lisible (`Box<int>`) alors qu'un `Type::Named("Box_int")` afficherait le nom mangé interne.
- Le mapping `(nom, args) → nom mangé` n'est de toute façon rempli que pour les instanciations que `collect_generic_instantiations` a pu voir (elle ne parcourt pas `program.generics` lui-même — les génériques imbriqués dépendants d'un paramètre de type externe à leur propre déclaration ne sont pas collectés), donc un renommage n'aurait de toute façon été que partiel.

## Ampleur (restante)

Aucune restante dans le périmètre "paramètre/champ/valeur de retour" — traité et vérifié ci-dessus. Limitations connues, distinctes, et déjà cataloguées ailleurs : l'échappement par argument ([memoire-echappement-argument](memoire-echappement-argument.md)) et l'absence de collecte des instanciations génériques imbriquées à l'intérieur d'un `generic` lui-même (mentionnée ci-dessus, jamais rencontrée en pratique — `examples/generics/` reste hors CI, voir [langage-syntaxe-obsolete](langage-syntaxe-obsolete.md)/[qualite-couverture-tests](qualite-couverture-tests.md)).

## Fichiers clés

`src/lower/builder.d/functions.rs` (paramètres), `src/lower/expr.d/helpers.rs` (`resolve_chained_field_class`), `src/lower/builder.d/class_ownership.rs` (`classify_field`), `src/main.rs` (E09 pour `program.generics`), `src/sema/typecheck.rs` (`substitute_type_params`, typecheck d'appel de méthode sur `Type::Generic`), `src/core/monomorph.rs`.
