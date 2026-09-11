# Couverture de tests et de CI incomplète

- `make regression`/`ci/regression.sh` ne couvre que `examples/[0-9][0-9]_*.oc`, `examples/project/main.oc` et `examples/builtins/*.oc` — **`examples/generics/`, `examples/from/`, `examples/mods/`, `examples/advanced/` ne sont exécutés par aucune cible Makefile/CI**.
- **Zéro test unitaire sur les génériques** (cohérent avec le fait que `examples/generics/` ne compile même plus, voir [langage-syntaxe-obsolete](langage-syntaxe-obsolete.md)).
- **Le seul test d'interfaces (`examples/tests/11_interfacesTest.oc`) a ses 4 assertions commentées** — il appelle les méthodes et retourne 0 sans vérifier aucune valeur, alors que 366 assertions `UnitTest::assertEquals` sont actives ailleurs dans la suite.
- `consumed` n'a aucun test dédié.
- Aucune infrastructure d'intégration MySQL en CI (pas de service/container documenté) — explique l'échec du test `builtins/mysql` en local par absence de serveur, indépendamment du bug de remontée d'erreur (voir [builtins-erreurs-incoherentes](builtins-erreurs-incoherentes.md)).

## Ampleur

Léger à moyen : étendre les cibles Makefile/CI aux dossiers actuellement exclus, réactiver de vraies assertions sur le test d'interfaces, ajouter des tests dédiés à `consumed` et aux génériques, ajouter un mécanisme CI pour MySQL (service container ou test explicitement skip-able).
