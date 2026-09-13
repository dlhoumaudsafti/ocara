# Interfaces

## ✅ Deux bugs corrigés

- **Import à symbole unique et interfaces transitives** : `import Circle from "fichier"` ne rapatriait que la classe nommée, sans jamais regarder sa liste `implements` pour ramener les interfaces qu'elle référence dans le même fichier source (`src/main.rs`). Confirmé par `examples/tests/11_interfacesTest.oc.bak`, qui contournait le problème en redéfinissant localement les interfaces plutôt que de les importer. **Corrigé** : le chargement d'une classe importée par son nom rapatrie désormais automatiquement toute interface de sa liste `implements` trouvée dans le même fichier.
- **Vérification de signature absente pour E09** : le check d'implémentation d'interface (`src/main.rs`) ne vérifiait que la présence d'une méthode du même nom, jamais sa signature. **Corrigé** : arité, types des paramètres et type de retour sont maintenant comparés (via `types_compat`/`type_name`), avec un message d'erreur dédié en cas de désaccord.

Vérifié manuellement (scripts de test ad hoc) : une classe qui implémente une interface définie dans le même fichier importé compile désormais sans avoir à réimporter l'interface explicitement ; une méthode dont la signature ne correspond pas à l'interface est maintenant rejetée à la compilation. `make regression` ne montre aucune régression.

## Toujours ouvert : aucun polymorphisme réel à l'exécution

La grammaire des interfaces reste minimale (signatures seules, pas d'héritage d'interface, pas de méthode par défaut, pas de champs — `src/parsing/ast.d/interfaces.rs:11-28`). Le dispatch est **100 % statique par mangling de nom** — aucune vtable. Une variable typée par une interface (`var d:Drawable`) mangerait ses appels vers une fonction jamais générée, puisque seules les classes concrètes émettent du code. L'opérateur `is` ne distingue d'ailleurs pas une classe d'une interface : `x is Circle` et `x is Drawable` compilent en code strictement identique.

## Ampleur

Le polymorphisme réel à l'exécution (tag de type par objet ou vtable, propagation dans l'IR et le codegen) reste un chantier architectural : actuellement 0 % de cette mécanique n'existe.

## Fichiers clés

`src/parsing/ast.d/interfaces.rs`, `src/main.rs`, `src/codegen/` (absence totale de logique d'interface).
