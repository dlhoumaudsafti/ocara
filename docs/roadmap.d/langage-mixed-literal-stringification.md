# `mixed` : un angle mort restant, plus profond que prévu

Le stockage des littéraux `float`/`bool` dans un `array`/`map` (boxing au lieu de stringification), le boxing des arguments d'appel, et `IO::writeln(JSON::encode(x))`/`obj.encode()` sans variable intermédiaire (`expr_ir_type` reconnaît maintenant `Ptr` par défaut pour un appel non résolu — vérifié directement : les deux formes affichent maintenant le JSON correct) sont corrigés — voir git log.

## Reste à faire (Structurel, pas Simple comme espéré) : `value_to_json`/`value_to_yaml` confondent un entier brut `0`/`1` avec un booléen/null

`array<int> = [1, 2, 3]` encodé en JSON affiche `[true,2,3]` — et `array<int> = [0, ...]` affiche même `null` pour l'élément `0` (`value_to_json` traite tout `val == 0` comme null avant même d'atteindre l'heuristique bool). En creusant pour corriger ça maintenant que le boxing des arguments `mixed` est réglé : **ce n'est pas une simple heuristique à retirer**. `OcaraArray`/`OcaraMap` (`runtime/src/lib.rs`) sont de simples `Vec<i64>`/`Vec<(String, i64)>` — aucune information de type par élément ne survit à l'exécution pour un conteneur **concret** (`array<int>`, `array<bool>`), qui ne boxe jamais ses éléments (contrairement à `array<mixed>`, déjà boxé et correctement encodé). `0` (int concret), `null`, et `false` (jamais boxé) partagent donc le même bit pattern à ce point du code, sans aucun moyen de les distinguer une fois qu'on est dans `value_to_json`/`value_to_yaml` — l'ambiguïté ne vient pas d'un oubli de boxing mais d'une vraie absence d'information de type au runtime pour les conteneurs concrets.

Deux vraies pistes, plus larges qu'un correctif Simple :
- faire porter l'information de type concret de l'élément jusqu'à l'appel `JSON::encode`/`YAML::encode` (un paramètre supplémentaire, ou une variante de fonction générée par élément concret) ;
- ou boxer aussi les éléments d'un conteneur concret (perdrait l'optimisation actuelle qui évite le coût de boxing pour `array<int>`/`array<bool>`).

## Fichiers clés

`runtime/src/lib.rs` (`value_to_json`, `__is_bool`, `OcaraArray`/`OcaraMap`), `runtime/src/yaml.rs` (`value_to_yaml`).
