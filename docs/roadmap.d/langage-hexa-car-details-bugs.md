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

**Correctif** : `src/lower/stmt.d/element_escape.rs`. Un conteneur dont un
élément est conservé (initialiseur, affectation, `return`, argument d'appel,
littéral — via un index, un champ ou une variable de boucle qui le parcourt)
n'est libéré qu'en surface (`__array_free_shallow`/`__map_free_shallow`) :
ses éléments fuient, jamais de use-after-free. Une lecture transitoire
(template, comparaison) garde la libération profonde.

**Limite** : prudent par nom sur tout le corps de la fonction, et toute
extraction conservée fait passer le conteneur en libération de surface
(fuite des éléments plutôt que copie).

## 5. Littéral passé en argument construit en `array<mixed>`

`use D([1.5, 2.0])` avec `prices:array<float>` stockait des flottants
« boxés », relus comme des flottants bruts (valeurs aberrantes).
**Correctif** : `IrModule::param_ast_types` (types AST des paramètres des
fonctions, méthodes et constructeurs) et `lower_call_arg` (`lower.rs`) : un
littéral `[...]`/`{...}` est construit au type déclaré du paramètre.

## Tests

`examples/tests/79_field_loop_match_and_kept_elementsTest.oc`, tests
unitaires dans `element_escape.rs`.
