# Les littéraux `float`/`bool` dans un conteneur `mixed` sont stockés comme des strings

## Constat

Découvert en travaillant sur le support YAML (`docs/roadmap.d/builtins-yaml.md`). Dans `src/lower/expr.d/lower.rs`, le lowering d'un littéral `array`/`map` convertit explicitement tout élément/valeur de type `float` ou `bool` en string (`__str_from_float`/`__str_from_bool`) **avant** de le stocker, dès que le conteneur est `mixed` — commentaire du code : *« Convertit F64/Bool en string avant stockage (comme pour les arrays) »*. Seul `int` est stocké tel quel.

## Reproduction

```ocara
const data:map<string, mixed> = { 'pi': 3.14159, 'count': 42, 'active': true }
IO::writeln(JSON::encode(data))
// {"active":"true","count":42,"pi":"3.14159"}
//           ^^^^^^              ^^^^^^^^^^^
//  bool ET float rendus comme des chaînes JSON, alors que count (int) reste un vrai nombre
```

Le même résultat s'observe avec `YAML::encode` (`pi: '3.14159'`, `active: 'true'` — tous deux quotés comme des chaînes). Ce n'est pas un bug de YAML/JSON eux-mêmes : ils reçoivent fidèlement une string en entrée, là où un int/un vrai booléen/float boxé aurait été fidèlement rendu comme un nombre/booléen.

## Portée

Touche potentiellement **tout consommateur de valeurs `mixed`** construites via un littéral `array`/`map` — pas seulement YAML/JSON. Un flottant ou un booléen qui *arrive* déjà boxé dans un `mixed` par un autre chemin (résultat de requête SQL via `__box_float`, valeur déjà décodée d'un YAML/JSON) n'est PAS affecté — seule la construction d'un littéral directement dans le code source Ocara l'est.

## Pourquoi ce n'est probablement pas un simple correctif

Le commentaire du code suggère un choix délibéré, pas un oubli — vraisemblablement parce que le mécanisme de boxing (`__box_float`/`__box_bool`, 2 bits de tag sur un pointeur aligné) n'était pas encore en place ou pas jugé fiable au moment de l'écriture de ce chemin de lowering. Le corriger proprement (boxer `float`/`bool` au lieu de les stringifier) demande de vérifier tous les consommateurs qui lisent aujourd'hui une valeur `mixed` en supposant implicitement qu'un float/bool y est une string (recherche à faire dans `src/builtins/`, `runtime/`) — un changement de représentation qui déborde du seul lowering.

## Fichiers clés

`src/lower/expr.d/lower.rs` (`Expr::Map`, `Expr::Array` — recherche `__str_from_float`/`__str_from_bool`).
