# Page « détail voiture » de mini_project_hexa : 5 bugs du compilateur — corrigé

Signalé sur `examples/advanced/mini_project_hexa` : l'historique des
entretiens affichait l'id à la place de la description, une date au
01/01/1970, un coût de 0,00 €, et « Entretien » pour toutes les lignes (pas
de distinction avec « Amélioration »). Le code de l'exemple était correct.

## 1. Boucle sur un champ : chaque champ lu à l'offset 0

`for m in car_details.maintenances` — la boucle porte sur un champ, pas une
variable. Le type d'élément n'était cherché que pour une variable
(`elem_ast_types`). `m` n'avait donc pas de classe, et `m.description`,
`m.cost`… lisaient tous le premier champ (`id`).

**Correctif** : `lower_for_in`/`lower_for_map`
(`src/lower/stmt.d/statements.d/loops.rs`) utilisent `elem_type_after_index`,
qui couvre variable, champ, appel et index.

## 2. Champ d'un élément indexé : même bug

`items[0].name` lisait le premier champ. **Correctif** :
`resolve_receiver_class` (`src/lower/expr.d/helpers.rs`) gère `Expr::Index`.

## 3. `match`/`switch` sur une chaîne comparée par adresse

Un motif `"improvement"` ne reconnaissait que le même littéral, jamais une
chaîne lue en base ou construite (`"improve" + "ment"`) : toujours
`default`. **Correctif** : `emit_pattern_eq` (`helpers.rs`) compare les
chaînes par valeur (`__cmp_eq_strict`), utilisé par `match` (`lower.rs`) et
`switch` (`control_flow.rs`).

## 4. Éléments conservés d'un conteneur `consumed`/`scoped` libérés avec lui

`MaintenanceRepository::forCar` : `consumed rows` puis
`items.push(self::fromRow(row))`, où `fromRow` extrait les chaînes de `row`
dans l'entité. À la fin de la méthode, `rows` était libéré en profondeur :
les chaînes de l'entité devenaient pendantes (description vide, puis SIGSEGV
sur une reproduction minimale).

**Correctif** (`src/lower/stmt.d/element_escape.rs`), sans fuite :
- **Copie** : une chaîne DÉRIVÉE (index, champ, variable de boucle) conservée
  vers une cible `string` est copiée (`__value_dup_leaf`, runtime) :
  - déclaration `var s:string = row["name"]` ;
  - affectation à une variable libérée, à `self.champ`, ou à un élément d'un
    conteneur de `string` ;
  - argument d'un paramètre `string` que l'appelé conserve ;
  - élément de littéral.

  Les chaînes sont immuables : la copie est invisible. Une copie non conservée
  est libérée en fin de bloc (`var` libéré automatiquement :
  `is_copied_string`, `ownership.rs`).
- **Ce que l'appelé conserve** : `IrModule::param_keeps` (point fixe sur
  tout le programme). `fromRow(row)` ne conserve que des copies : passer `row`
  ne compte pas comme conservation.
- **Alias local** : `var first:map<…> = rows[0]` est traité comme une
  variable de boucle (alias). Une `consumed` aliasée n'est libérée qu'en fin
  de bloc (`var_alias_roots`).
- **Builtins purs** (`IO::writeln`, `Convert::*`, `Math::*`, `String::*`,
  `JSON::encode`…, `crate::sema::escape::is_pure_builtin`) : ne conservent
  rien.
- **Libération de surface** seulement en dernier recours, pour un élément
  composite (tableau, map, objet) réellement conservé, ou retourné : pas de
  copie au `return`, sinon une méthode d'accès copierait à chaque appel.

Vérifié par mesure mémoire (20 000 puis 200 000 appels, stable à ≈ 2 Mo) et
dans l'IR : `forCar` libère `rows` en profondeur (`__value_free`), `fromRow`
copie les chaînes.

### Fuite corrigée au passage : clés de map

`__map_set` recopie la clé dans la map ; toutes les clés allouées par le
runtime pour l'insertion (`alloc_str(nom)`) fuyaient. Cela concernait
`JSON::decode`, `YAML`, chaque ligne SQLite/MySQL, les en-têtes
HTTP, `HTMLComponent`, `File`/`Directory`, `Map::clone`…
**Correctif** : `map_set_owned_key` (`runtime/src/lib.rs`) insère puis libère la
clé ; utilisé par tous ces appels (pas par `Map_set`, dont la clé appartient
au programme). Mesure : `JSON::decode` d'un objet passait de 2 Mo à 40 Mo
pour 200 000 appels, il reste stable à 2 Mo.

### Reste ouvert

Objets d'un conteneur `scoped` jamais libérés →
[memoire-scoped-object-elements-leak](memoire-scoped-object-elements-leak.md).

## 5. Littéral passé en argument construit en `array<mixed>`

`use D([1.5, 2.0])` avec `prices:array<float>` stockait des flottants
« boxés », relus comme des flottants bruts (valeurs aberrantes).
**Correctif** : `IrModule::param_ast_types` (types AST des paramètres des
fonctions, méthodes et constructeurs) et `lower_call_arg` (`lower.rs`) : un
littéral `[...]`/`{...}` est construit au type déclaré du paramètre.

## Tests

`examples/tests/79_field_loop_match_and_kept_elementsTest.oc`, tests
unitaires dans `element_escape.rs`.
