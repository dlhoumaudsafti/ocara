# Architecture hexagonale — vérification compile-time des dépendances

## Proposition

Faire de l'architecture hexagonale la structure par défaut d'un programme Ocara. Le développeur peut la déclarer explicitement dans le fichier d'entrée, notamment pour configurer les noms de répertoires, ou demander `architecture permissive` pour désactiver les contrôles architecturaux. La structure permissive ne relâche ni le typage strict ni les autres règles du langage.

L'objectif est de faire de la structure hexagonale un choix naturel et vérifié pour les programmes Ocara, sans empêcher les scripts et programmes qui ne souhaitent pas ces contraintes architecturales. Le compilateur signale les dépendances interdites avec des diagnostics précoces, plutôt que de laisser ces règles à une revue de code ou à un outil externe tel que Deptrac.

La structure hexagonale est le défaut; sa déclaration explicite peut préciser les alias de répertoires :

```ocara
architecture hexagonal
```

```ocara
architecture hexagonal {
    infrastructure: infra,
    application: app,
    domain: dom
}
```

Les noms situés à droite des deux-points sont des alias de répertoire. Les noms canoniques des couches sont `infrastructure`, `application` et `domain`. Les alias `infra`, `app` et `dom` sont des choix de projet, pas des mots-clés du langage. Une couche omise conserve son nom canonique :

```ocara
architecture hexagonal {
    infrastructure: infra,
    application: app
}
```

Dans cet exemple, les répertoires sont `infra/`, `app/` et `domain/`.

## Règles d'import

La direction des dépendances est orientée vers les couches plus internes :

| Couche du fichier qui importe | Couches ordinaires autorisées |
|---|---|
| `domain` | `domain` |
| `application` | `application`, `domain` |
| `infrastructure` | `infrastructure`, `application`, `domain` |

Le compilateur rejette donc les imports ordinaires suivants :

- depuis `domain/` vers `application/` ou `infrastructure/` ;
- depuis `application/` vers `infrastructure/`.

Les imports d'une même couche restent autorisés, sous réserve des règles de contexte ci-dessous. Un fichier d'infrastructure peut dépendre des contrats du domaine et des services applicatifs, par exemple un contrôleur HTTP qui appelle un cas d'usage.

La règle vise les dépendances entre fichiers Ocara effectivement résolues par le compilateur, pas le seul texte écrit après `import`. La vérification doit donc s'appliquer après résolution du chemin réel, aux imports qualifiés, relatifs, aliasés et aux imports transitifs chargés par le compilateur. Les imports des modules builtins `ocara.*` ne sont pas classés comme couches du projet.

## Contextes maîtres

Une racine de contexte représente un sous-arbre architectural cohérent, par exemple `context.home` ou `context.car`. Dans chacun de ces contextes, les segments de couche sont reconnus selon les alias déclarés :

```text
context.home.domain.contract.CarContract
context.home.app.usecase.CarService
context.home.infra.db.CarRepository
```

Deux contextes non partagés sont isolés par défaut. Un import de `context.car.infra.db.CarRepository` depuis `context.home.infra.controller.HomeController` est donc rejeté, même si les deux fichiers appartiennent à `infrastructure`. L'égalité de couche n'autorise pas à elle seule une dépendance entre deux contextes maîtres distincts.

Les chemins source et les namespaces doivent rester cohérents. Le compilateur doit classer un module à partir du chemin résolu et vérifier que son namespace ne contredit pas ce classement; sinon, un namespace trompeur pourrait contourner les frontières de dossiers. La règle précise de gestion des fichiers dont le namespace ne reflète pas leur chemin reste à définir : diagnostic dédié recommandé en mode `hexa`.

## Racines partagées

La propriété `shared` déclare les racines transversales accessibles depuis les contextes du projet :

```ocara
architecture hexagonal {
    infrastructure: infra,
    application: app,
    shared: [
        context.core,
        context.shared
    ]
}
```

Une racine partagée est une exception aux frontières entre contextes, pas une exemption automatique aux règles de couches. Si `context.core` suit lui-même la structure hexagonale, ses modules restent classés par couche :

- `context.home.app.usecase.HomeService` peut importer `context.core.domain.contract.Clock` ;
- il ne peut pas importer `context.core.infra.db.ClockRepository` ;
- `context.home.domain` ne peut pas importer un module de couche `app` ou `infra` de `context.core`.

Ainsi, déclarer une racine partagée n'autorise pas `application` à dépendre de l'infrastructure d'un autre contexte. Les deux contrôles sont cumulatifs : autorisation de franchir la frontière de contexte, puis validation de la direction entre couches.

Les racines partagées sans segment de couche identifiable, telle que `configs`, sont un cas à trancher. La proposition initiale les utilise notamment pour autoriser certains `wiring`. Il faut préciser si elles sont :

1. uniquement accessibles dans les déclarations `wiring` ;
2. importables par toutes les couches comme modules de composition ;
3. soumises à une catégorie explicite supplémentaire (par exemple `composition:` ou `shared` avec une politique dédiée).

Par prudence, elles ne devraient pas être considérées silencieusement comme un accès libre pour les imports ordinaires.

## Règles de `wiring`

`wiring` reste une déclaration de liaison compile-time, différente d'un import ordinaire. Il permet au code consommateur de dépendre du contrat, tandis que le compilateur connaît l'adaptateur concret sans exiger que le service applicatif l'importe.

Dans un contexte maître donné, une interface ne peut référencer par `wiring` qu'une implémentation appartenant au même contexte maître. Par exemple, une interface sous `context.home.domain` peut viser une classe sous `context.home.infra` ou `context.home.app`, `context.home.domain`, mais pas une classe sous `context.car.*`.

Une racine déclarée dans `shared` peut également fournir une cible de `wiring`, conformément au cas d'usage demandé :

```ocara
architecture hexagonal {
    infrastructure: infra,
    application: app,
    shared: [context.core, context.shared, configs]
}
```

Dans cette configuration, un `wiring` déclaré dans `context.home.domain` peut cibler une implémentation de `context.home`, `context.core`, `context.shared` ou `configs`. Cette autorisation concerne le lien de compilation et ne rend pas automatiquement ces modules importables depuis le code du domaine.

L'architecture hexagonale ne doit pas modifier la résolution existante de `wiring` (choix du premier wiring sans alias, sélection par alias, conformité `implements`, etc.). Elle ajoute une vérification de frontière à ces règles. La cible doit toujours être résolue et satisfaire les vérifications existantes.

## Répertoires de projet et configuration

La déclaration d'architecture est placée une fois dans le fichier d'entrée du programme et s'applique au graphe complet des sources utilisateur atteint depuis ce point d'entrée. Une bibliothèque importée ne remplace pas silencieusement la politique architecturale du programme appelant. En l'absence de déclaration, la structure hexagonale s'applique avec les noms de couches canoniques.

Le format exact et le support des racines de contexte explicites restent à décider. Deux stratégies sont envisageables :

- déduire les contextes depuis une racine convenue, telle que `context.<nom>.<couche>` ;
- déclarer aussi les racines de contexte dans `architecture hexagonal`, pour permettre plusieurs conventions de répertoires.

La première approche est simple et correspond au projet `mini_project_hexa`. La seconde est plus flexible, mais augmente la configuration. La conception devra également définir le comportement des fichiers de l'entrée principale, des tests, des scripts sans namespace, et des modules partagés imbriqués sous un contexte.

## Diagnostics attendus

Les violations doivent être des erreurs de compilation localisées sur l'import ou le `wiring` fautif. Le diagnostic devrait indiquer la couche et le contexte source, la cible résolue, et la règle violée, par exemple :

```text
error: dependency from 'context.home.app' to 'context.home.infra.db' is forbidden by architecture 'hexagonal'
       application may depend on application and domain, but not infrastructure
```

Cas à diagnostiquer explicitement :

- import interdit entre couches d'un même contexte ;
- import entre contextes non partagés ;
- import vers une racine partagée dont la couche viole la direction ;
- `wiring` hors du contexte maître de l'interface et hors des racines partagées ;
- alias de couche dupliqué ou invalide ;
- fichier ou namespace impossible à classifier sans ambiguïté ;
- entrée de `shared` qui ne correspond à aucune racine source résolue.

Les erreurs de configuration architecturale devraient être distinguées des erreurs de syntaxe Ocara ordinaires afin de faciliter leur compréhension et les tests.

## Architecture permissive et compatibilité

- L'absence de déclaration équivaut à `architecture hexagonal` avec les noms canoniques `infrastructure`, `application` et `domain`.
- `architecture permissive` désactive uniquement les contrôles de couches, de contextes et de frontières de `wiring`; le typage strict, les règles de visibilité et toutes les autres vérifications restent actifs.
- L'architecture hexagonale est stricte : toute dépendance utilisateur que le compilateur ne peut pas classer doit produire un diagnostic plutôt que d'être acceptée par défaut.
- Ce défaut strict peut rendre non compilables des projets Ocara existants qui ne suivent pas une arborescence hexagonale. Ils devront soit adopter ou adapter cette structure, soit déclarer `architecture permissive` pour conserver leur organisation actuelle.
- Les builtins `ocara.*` ne sont pas concernés par les règles de couches.
- Les vérifications architecturales n'introduisent aucun coût à l'exécution : elles sont faites à la compilation.

## Tests de non-régression à prévoir

1. Absence de déclaration et déclaration explicite `architecture hexagonal` appliquent les mêmes contrôles stricts; `architecture permissive` accepte les graphes indépendamment de leurs couches.
2. Alias de couches personnalisés et couches canoniques omises sont correctement interprétés.
3. Les six directions pertinentes entre couches sont testées, notamment `domain → app/infra` et `app → infra` en rejet, et `infra → app/domain` en acceptation.
4. Un import inter-contexte est rejeté sans `shared`, puis accepté avec la racine déclarée si sa couche est autorisée.
5. Un import depuis `app` vers `context.core.domain` est accepté, vers `context.core.infra` rejeté.
6. Le `wiring` dans le même contexte est accepté; celui vers un autre contexte est rejeté; une racine `shared` peut être une cible de wiring sans ouvrir les imports ordinaires.
7. Les mêmes contrôles s'appliquent aux imports aliasés, relatifs, wildcard/sélectifs si ces formes sont autorisées, et transitifs.
8. Les namespaces qui contredisent leur chemin, les collisions d'alias et les chemins non classifiables donnent des diagnostics stables.
9. Une application représentative, notamment `examples/advanced/mini_project_hexa`, compile sous le mode choisi et sert de test d'intégration architectural.

## Décisions ouvertes avant implémentation

- Syntaxe définitive de `architecture hexagonal` et de la liste `shared` (virgules, séparateurs, plusieurs blocs ou un seul bloc global).
- Racine de contexte déduite ou déclarée; gestion de plusieurs projets/racines dans un même graphe.
- Politique des racines `shared` non stratifiées telles que `configs`, séparée pour les imports ordinaires et les `wiring`.
- Une racine partagée peut-elle contenir des couches, et comment les reconnaître lorsqu'elle est nommée `context.shared` ?
- Traitement des tests, outils, fichiers d'entrée hors contexte et bibliothèques utilisateur externes.
- La validation porte-t-elle uniquement sur les imports directs (recommandé pour des diagnostics lisibles) ou aussi sur une fermeture transitive, sachant qu'un import direct constitue déjà le graphe de dépendances visible au compilateur ?
- Diagnostic ou erreur fatale lorsque la source ne peut pas être classée en mode strict.

## Priorité / Complexité

**Priorité Moyenne** — structure hexagonale stricte par défaut, avec opt-out explicite via `architecture permissive`; apporte une vérification architecturale aux applications métier tout en préservant un chemin sans ces contraintes pour les autres programmes. **Complexité estimée : Structurelle** — configuration de projet, classification des sources, intégration au graphe d'import, diagnostics et contraintes spécifiques à `wiring`; la résolution actuelle s'appuie sur namespaces et chemins, et nécessite une vérification centralisée pour éviter des règles divergentes entre le chargement normal des imports et le pré-scan de `wiring`.

## Fichiers et surfaces concernés

À confirmer lors de l'implémentation : grammaire et AST du programme (`src/parsing/`), chargement/résolution des imports (`src/main.rs`, `src/core/interface_wiring.rs`), validations sémantiques et diagnostics (`src/sema/`, `src/core/`), tests du parser/imports/`wiring`, documentation canonique (`docs/EBNF.md`, §31 compris), ainsi que l'exemple `examples/advanced/mini_project_hexa` et sa commande de build.
