# Variable capturée par une fermeture : métadonnées perdues — corrigé

Audit du 2026-10-19, corrigé le 2026-10-20.

## Constat

Dans le corps d'une `nameless`, une variable capturée ne gardait que sa
classe (`var_class`) :

- map capturée lue ou écrite par index (`m["a"] = 5`, `m["a"] += 1`,
  `m["a"]++`, `map<int, int>`, `arr[0]["k"] = v`) : traitée comme un tableau
  (`__array_set`/`__array_get`), SIGSEGV ou valeur fausse — le partage d'une
  map entre threads, handlers HTTP et `withLock` était cassé ;
- fonction capturée rendant une `string`, appelée dans la fermeture : une
  chaîne perdue par appel.

## Correctif

`CaptureMeta` (`src/lower/expr.d/nameless.rs`) copie, au moment de la
création de la fermeture, les tables de la fonction englobante (classe, map,
type de clé, type et type AST des éléments, nullabilité, fonction et son type
de retour) ; chaque nom capturé y retrouve les siennes (`CaptureMeta::apply`),
fermetures imbriquées comprises.

## Tests

`examples/tests/100_closure_captured_metadataTest.oc` (écriture et lecture,
`+=`/`++`, clés `int`, map dans un tableau, `array<float>`, fonction capturée,
fermeture imbriquée, map partagée entre 4 threads sous `withLock`) ;
`--leak-check` sans fuite.
