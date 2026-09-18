# `property` de type ressource sur une classe utilisateur

## Terminé — option 1 retenue et implémentée (analyse d'échappement étendue)

Voir §"Ce qui a été fait" plus bas.

## Constat (avant correctif)

Une `property` de type `Mutex`/`SQLite`/`MySQL`/`MariaDB`/`HTTPRequest`/`HTTPResponse` sur une classe utilisateur était **rejetée à la compilation** (diagnostic E29, `docs/diagnostics.md` §E29) :

```
fichier.oc:5:5: error: 'Cache.lock' ('Mutex') is a native resource field — it is never closed when a 'Cache' instance is destroyed (no mechanism exists for this today), so this handle always leaks; manage it outside the class instead, or expose an explicit method the caller must invoke before discarding the instance
```

Raison : `__free_<Classe>` (généré pour une instance `scoped`/`consumed`, ou un `var` dont l'échappement est prouvé impossible — voir `docs/EBNF.md` §9) ne savait libérer qu'un champ `string`/`array`/`map`/instance de classe utilisateur, jamais fermer une ressource native.

Cas concret qui a motivé cette fiche : `examples/advanced/tauri_httpserver/configs/Database.oc` voulait garder une connexion `SQLite` ouverte comme champ d'instance (`private property db:SQLite`, ouverte une fois dans `init()`, réutilisée par `migrate()`/`recordVisit()`) — pattern d'encapsulation courant (« la classe possède sa ressource »).

## Ce qui a été fait

### Option retenue : réutiliser l'analyse d'échappement existante, étendue aux classes composites

Des trois pistes envisagées initialement, l'option 1 (« une classe contenant une ressource EST une ressource, partout où `ownership_class` est consultée ») a été retenue — la plus cohérente : elle évite une règle hybride (« résource pour l'échappement, valeur pour l'auto-libération ») difficile à justifier et à vérifier dans tous les coins.

1. **`crate::sema::scope::compute_resource_classes`** (nouveau) : calcule, par point fixe sur `program.classes`, l'ensemble des classes contenant (directement ou transitivement, via un autre champ de classe) une ressource native. Calculé une fois dans `TypeChecker::check_program`, stocké dans `self.resource_classes`.
2. **`crate::sema::scope::ownership_class_of(ty, resource_classes)`** (nouveau) : comme `ownership_class`, mais retourne `OwnershipClass::Resource` pour un `Type::Named` présent dans `resource_classes`. Remplace `ownership_class` aux 4 sites de `typecheck.rs` qui décident d'un comportement d'échappement/possession (`check_class` pour l'ancien rejet E29 — supprimé, `check_resource_var_containment`, `check_escape`, `check_argument_escape`) et dans `pop_scope`/`resource_raise::check_program` (W04), désormais tous conscients des classes composites.
3. **Conséquence directe, sans code supplémentaire** : une instance `scoped`/`consumed` d'une classe-ressource ne peut plus s'échapper de son bloc (E18) — empêche par construction que deux instances partagent le même handle natif via un clonage. Un `var`/`const` non fermé et prouvé non-échappant est rejeté (E28) — une classe composite n'ayant en général pas de méthode `close()` à elle, `scoped`/`consumed` reste en pratique le seul choix.
4. **`crate::lower::builder::class_ownership`** : `FieldOwnership` gagne une variante `Resource(String)` ; `build_free_function` émet l'appel de fermeture natif (`crate::sema::scope::resource_closer_symbol`, mapping partagé avec `lower::stmt::ownership::drop_func_for` pour éviter deux copies divergentes) ; `build_clone_function` ne clone JAMAIS un champ ressource (mettrait en pratique deux instances face au même handle) — écrit un `0` défensif, chemin qui ne devrait de toute façon jamais être atteint par un programme qui compile, l'échappement étant bloqué en amont.
5. **Nouveau diagnostic E35 (`SemaError::ManualCloseOnResourceField`)** : un appel manuel `.close()`/`.destroy()`/`.closeResponse()` sur `self.<champ>` est rejeté — `__free_<Classe>` le fermera de toute façon à la destruction de l'instance, un appel en plus serait toujours une double fermeture (pas de suivi inter-méthodes possible pour distinguer un usage sûr, contrairement à E25 qui ne détecte qu'un DEUXIÈME appel explicite dans le même bloc).

### Vérifications

- `cargo test -p ocara` : 86 passed (dont 5 nouveaux tests dédiés, `src/sema/tests/resource_property_destructor.rs` : déclaration acceptée, échappement d'un `scoped` toujours rejeté, `var` non-échappant toujours rejeté, fermeture manuelle sur `self.<champ>` rejetée, classe SANS ressource inchangée).
- `make regression` (cache vidé) : 677 PASS, 0 FAIL, 0 ERREUR — dont un nouvel exemple de bout en bout (`examples/tests/52_class_resource_property_destructorTest.oc`) : trois instances successives d'une classe `Database` (connexion `SQLite` en `property`) sur le même fichier, sans jamais fermer manuellement — si la fermeture automatique ne fonctionnait pas, la connexion suivante resterait bloquée par le verrou d'écriture SQLite (même famille de symptôme que `49_sqlite_with_open_raiseTest.oc`).
- `make build` : 0 warning.
- `examples/advanced/tauri_httpserver/configs/Database.oc` compile désormais tel que l'utilisateur l'avait initialement écrit (`private property db:SQLite`, ouverte dans `init()`, jamais fermée manuellement par `migrate()`/`recordVisit()`).

## Limite connue, documentée mais non bloquante

Le mécanisme couvre les instances `scoped`/`consumed` et `var`/`const` prouvées non-échappantes. Une instance qui s'échappe réellement (retournée, stockée dans un champ d'une autre classe, passée en argument retenu) reste rejetée à la compilation (même traitement que E18 pour une ressource nue) plutôt que silencieusement mal géré — pas de fuite ni de double-fermeture possible, seulement une contrainte d'usage assumée (cohérente avec le reste du langage).

## Fichiers clés

`src/sema/scope.rs` (`compute_resource_classes`, `ownership_class_of`, `resource_closer_symbol`), `src/sema/typecheck.rs` (suppression de l'ancien rejet E29, nouveau E35), `src/sema/error.rs`, `src/sema/resource_raise.rs`, `src/lower/builder.d/class_ownership.rs` (`FieldOwnership::Resource`), `src/lower/stmt.d/ownership.rs` (`drop_func_for`, refactoré pour partager `resource_closer_symbol`), `src/sema/tests/resource_property_destructor.rs`, `examples/tests/52_class_resource_property_destructorTest.oc`, `examples/advanced/tauri_httpserver/configs/Database.oc`, `docs/diagnostics.md` (§E29 réécrit, §E35 nouveau).
