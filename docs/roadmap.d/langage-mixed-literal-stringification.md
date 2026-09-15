# `mixed` : deux angles morts restants autour du typage dynamique

Le stockage des littéraux `float`/`bool` dans un `array`/`map` (boxing au lieu de stringification) et le boxing des arguments `float`/`bool`/`int` d'un appel (fonction libre, constructeur, méthode d'instance/statique) sont corrigés — voir git log pour le détail.

## Reste à faire : `value_to_json`/`value_to_yaml` confondent un entier brut `0`/`1` avec un booléen

`array<int> = [1, 2, 3]` encodé en JSON affiche `[true,2,3]` — l'heuristique (`__is_bool`) traite tout entier brut valant exactement 0/1 comme un booléen. Cette heuristique avait une justification **au moment où elle a été notée** : c'était le seul moyen de détecter, même imparfaitement, un booléen passé en argument `mixed` (jamais boxé à cette époque). **Cette justification ne tient plus** : le boxing des arguments `mixed` est maintenant corrigé (voir ci-dessus) — un vrai booléen transitant par un argument arrive maintenant correctement boxé et taggé. À réévaluer : l'heuristique brute 0/1-vaut-bool peut probablement être retirée maintenant sans rien casser, ce qui corrigerait au passage l'encodage `array<int>`.

## Reste à faire : `IO::writeln(JSON::encode(x))` affiche un nombre incohérent sans variable intermédiaire

`var s:string = JSON::encode(arr); IO::writeln(s)` fonctionne, mais `IO::writeln(JSON::encode(arr))` (appel direct, sans variable intermédiaire) affiche un nombre incohérent au lieu de la string JSON. Cause : `expr_ir_type` ne reconnaît pas `JSON_encode`/les méthodes d'instance (`x.encode()`) comme retournant `Ptr`, contrairement à `String_*`/`Array_join`/etc. déjà listés. Reproduit aussi pour un `array<int>` simple, sans rapport avec `mixed` — chantier séparé, dispatch de type des appels, pas le stockage des littéraux.

## Fichiers clés

`runtime/src/lib.rs` (`value_to_json`, `__is_bool`), `runtime/src/yaml.rs` (`value_to_yaml`), `src/lower/expr.d/typeinfer.rs` (`expr_ir_type`).
