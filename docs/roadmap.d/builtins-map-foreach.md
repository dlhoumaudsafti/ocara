# `Map::forEach` — implémenté

## Constat initial

`__map_foreach` (`runtime/src/lib.rs`) était un stub vide (« TODO : implémentation complète nécessite le support des pointeurs de fonctions ») — et, en creusant, **complètement mort** : aucun chemin de lowering ne l'appelait (ni les littéraux `{...}`, ni les boucles `for k => v in map`, contrairement à ce qu'affirmait la note dans `docs/builtins/Map.md`), et `forEach` n'était même pas enregistrée comme méthode appelable sur la classe builtin `Map` (`src/builtins/map.rs`).

## ✅ Corrigé

Le support des pointeurs de fonctions existe en réalité déjà dans le langage (fat pointer `{func_ptr, env_ptr}`, voir `HTTPServer::route`/`Thread::run`) — la seule pièce manquante était le branchement pour `Map`. Ajouté :

- `Map::forEach(m, callback)` enregistrée dans `src/builtins/map.rs` (signature) et `src/codegen/desc.d/map.rs` (descripteur Cranelift).
- `Map_forEach` implémentée dans `runtime/src/lib.rs` : extrait `{func_ptr, env_ptr}` du fat pointer (même convention que `runtime/src/httpserver.rs`), copie les entrées de la map (le callback peut modifier la map pendant l'itération) et appelle `func_ptr(env_ptr, key, value)` pour chacune.
- Le stub mort `__map_foreach` et son descripteur associé sont supprimés (remplacés, pas laissés en parallèle).
- Documenté dans `docs/builtins/Map.md` avec exemple d'utilisation et de capture de closure.

Vérifié manuellement : clé/valeur lues correctement, concaténation `key + " -> " + value` fonctionnelle, mutation d'une variable capturée (`Array::push` sur un `array` capturé) fonctionnelle. `make regression` sans régression.

**Découverte annexe en écrivant l'exemple de documentation** : l'arithmétique (`+`) entre un `int` et une valeur `mixed` contenant un entier produit un résultat faux — bug préexistant, indépendant de `forEach`, voir [langage-mixed-arithmetic](langage-mixed-arithmetic.md).

## Fichiers clés

`runtime/src/lib.rs` (`Map_forEach`), `src/builtins/map.rs`, `src/codegen/desc.d/map.rs`, `src/codegen/desc.d/lowlevel.rs` (retrait de `__map_foreach`), `docs/builtins/Map.md`.
