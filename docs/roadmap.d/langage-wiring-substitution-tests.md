# Remplacer un `wiring` depuis un test

Statut : **à concevoir** — demandé le 2026-10-17, en écrivant les tests de
`examples/advanced/mini_project_hexa`.

## Constat

Un contrat statique résolu par `wiring` (`CarContract::find(id)` dans un cas
d'usage) est lié à son implémentation **dans le fichier appelant** (EBNF
§17.1) : nom nu → premier `wiring`, alias d'import → le `wiring` de ce nom,
uniquement dans le fichier qui écrit l'alias. Un test ne peut donc pas faire
utiliser un faux dépôt par `CarDetailsService` sans modifier le code de
production (ajouter un `wiring` vers le faux dans l'interface et l'alias dans
le cas d'usage).

Ce n'est pas une impossibilité : un contrat à méthodes d'instance injecté en
paramètre (`CarDetailsService(cars:CarContract)`) accepte déjà un
`FakeCarRepository implements CarContract`, le type d'interface restant
abstrait. Le besoin est de garder la concision des contrats statiques tout
en isolant les couches `domain`/`app` de la base dans les tests.

## Proposition (à trancher)

Une déclaration réservée aux fichiers de test, qui remplace la cible d'un
`wiring` pour **tout le programme** compilé par `ocaraunit` :

```ocara
// tests/context/car/app/usecase/CarDetailsServiceTest.oc
import context.car.domain.contract.CarContract
import tests.support.FakeCarRepository

wiring CarContract to FakeCarRepository
```

Points à décider :
- syntaxe (`wiring X to Y` au niveau fichier, ou annotation sur la classe de
  test) ;
- portée : refusée hors d'un fichier `*Test.oc` (erreur de compilation), pour
  que le code de production ne puisse jamais l'utiliser ;
- vérifications identiques à un `wiring` ordinaire : `Y implements X` (E40),
  signatures compatibles (E09) ;
- architecture : le faux reste hors de `<src>` (pas de contrôle de couches),
  la substitution ne lève donc pas d'erreur de contexte ;
- interaction avec les alias d'import (E41) : la substitution s'applique au
  nom nu comme aux alias, ou seulement au nom nu.

## Validation

Un test de `CarDetailsService` avec un faux `CarContract` et un faux
`MaintenanceContract` en mémoire, sans `app.db` ; le même cas d'usage
compilé par `main.oc` utilise toujours `CarRepository`.

## Priorité / Complexité

**Basse** (confort de test, l'injection fonctionne déjà) — **Légère** à
**Structurel** selon la résolution du `wiring` (`core::interface_wiring`).
