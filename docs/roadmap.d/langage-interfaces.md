# Interfaces : aucun polymorphisme réel à l'exécution

## Constat

La grammaire des interfaces est minimale (signatures seules, pas d'héritage d'interface, pas de méthode par défaut, pas de champs — src/parsing/ast.d/interfaces.rs:11-28). Le dispatch est **100 % statique par mangling de nom** — aucune vtable (grep "vtable|dispatch|virtual" sur `src/` ne retourne rien). Une variable typée par une interface (`var d:Drawable`) mangerait ses appels vers une fonction jamais générée, puisque seules les classes concrètes émettent du code. Aucun exemple du dépôt ne déclare jamais de variable/champ/paramètre typé par une interface — confirmant que le polymorphisme runtime n'existe pas aujourd'hui. L'opérateur `is` ne distingue d'ailleurs pas une classe d'une interface : `x is Circle` et `x is Drawable` compilent en code strictement identique.

## Deux bugs isolés et rapides à corriger, indépendants du point ci-dessus

- **Import à symbole unique et interfaces transitives** : `import Circle from "fichier"` ne recherche que la classe nommée, sans jamais regarder sa liste `implements` pour rapatrier les interfaces qu'elle référence (src/main.rs:265-302). Confirmé par `examples/tests/11_interfacesTest.oc.bak`, qui contourne le problème en redéfinissant localement les interfaces plutôt que de les importer.
- **Vérification de signature absente pour E09** : le check d'implémentation d'interface (qui vit dans src/main.rs:420-448, pas dans `sema/`) ne vérifie que la présence d'une méthode du même nom, jamais sa signature (paramètres/type de retour) — `TODO` explicite non traité (src/main.rs:445).

## Ampleur

- Les deux bugs isolés : correctifs rapides et indépendants.
- Le polymorphisme réel à l'exécution (tag de type par objet ou vtable, propagation dans l'IR et le codegen) : chantier architectural, actuellement 0 % de cette mécanique n'existe.

## Fichiers clés

`src/parsing/ast.d/interfaces.rs`, `src/main.rs`, `src/codegen/` (absence totale de logique d'interface).
