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
    layers: {
        infrastructure: infra,
        application: app,
        domain: dom,
    },
}
```

Les couches sont regroupées dans `layers`, séparées des autres propriétés (`src`, `shared`). Les noms situés à droite des deux-points sont des alias de répertoire. Une virgule finale est permise après chaque entrée, y compris la dernière. Les noms canoniques des couches sont `infrastructure`, `application` et `domain`. Les alias `infra`, `app` et `dom` sont des choix de projet, pas des mots-clés du langage. Une couche omise conserve son nom canonique :

```ocara
architecture hexagonal {
    layers: {
        infrastructure: infra,
        application: app,
    },
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

La propriété `shared` déclare les racines transversales accessibles depuis les contextes du projet. Ses chemins partent de la racine du projet (dossier du fichier d'entrée), indépendamment de `src` : `context.core` désigne `context/core/`.

```ocara
architecture hexagonal {
    layers: {
        infrastructure: infra,
        application: app,
    },
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

Seuls les répertoires structurés en couches (segments `domain`, `application`, `infrastructure` ou leurs alias) peuvent être déclarés dans `shared`. Une entrée sans segment de couche, telle que `configs`, est une erreur de compilation.

## Règles de `wiring`

`wiring` reste une déclaration de liaison compile-time, différente d'un import ordinaire. Il permet au code consommateur de dépendre du contrat, tandis que le compilateur connaît l'adaptateur concret sans exiger que le service applicatif l'importe.

Dans un contexte maître donné, une interface ne peut référencer par `wiring` qu'une implémentation appartenant au même contexte maître ou à une racine `shared`. Par exemple, une interface sous `context.home.domain` peut viser une classe sous `context.home.infra`, `context.home.app` ou `context.home.domain`, mais pas une classe sous `context.car.*`.

Un contexte peut aussi dépendre d'un contrat partagé : une classe de `context.home` peut implémenter (`implements`) une interface déclarée dans une racine `shared`, par exemple `context.core.domain.contract.Clock`, dans le respect de la direction des couches. Le `wiring` de ce contrat partagé peut alors viser l'implémentation du contexte qui l'utilise.

Exemple de configuration :

```ocara
architecture hexagonal {
    src: mon_dossier_structuré_en_hexa, // par défaut le dossier src du projet : répertoire où le code est structuré en hexagonal
    layers: {
        infrastructure: infra,
        application: app,
    },
    shared: [context.core, context.shared],
}
```

```ocara
architecture hexagonal {
    src: context,
    layers: {
        infrastructure: infra,
        application: app,
        domain: dom,
    },
    shared: [context.core, context.shared],
}
```

Dans cette configuration, un `wiring` déclaré dans `context.home.domain` peut cibler une implémentation de `context.home`, `context.core` ou `context.shared`. Cette autorisation concerne le lien de compilation et ne rend pas automatiquement ces modules importables depuis le code du domaine.

L'architecture hexagonale ne doit pas modifier la résolution existante de `wiring` (choix du premier wiring sans alias, sélection par alias, conformité `implements`, etc.). Elle ajoute une vérification de frontière à ces règles. La cible doit toujours être résolue et satisfaire les vérifications existantes.

## Répertoires de projet et configuration

La déclaration d'architecture est placée une fois dans le fichier d'entrée du programme et s'applique au graphe complet des sources utilisateur atteint depuis ce point d'entrée. Une bibliothèque importée ne remplace pas silencieusement la politique architecturale du programme appelant. En l'absence de déclaration, la structure hexagonale s'applique avec les noms de couches canoniques.

La propriété `src` désigne le répertoire soumis à la structure hexagonale (par défaut `src/`) ; le reste du projet n'est pas contrôlé. Les contextes sont déduits sous ce répertoire : `<src>.<contexte>.<couche>`. Dans `mini_project_hexa`, seul `context/` est structuré en hexagonal (`src: context`) ; `configs/`, `core/` et `templates/` restent libres.

```ocara
architecture hexagonal {
    src: context,
    layers: {
        infrastructure: infra,
        application: app,
    },
}
```

La conception devra également définir le comportement des fichiers de l'entrée principale, des scripts sans namespace, et des modules partagés imbriqués sous un contexte.

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
- entrée de `shared` qui ne correspond à aucune racine source résolue ;
- entrée de `shared` sans segment de couche (ex. `configs`).

Les erreurs de configuration architecturale devraient être distinguées des erreurs de syntaxe Ocara ordinaires afin de faciliter leur compréhension et les tests.

## Architecture permissive et compatibilité

- L'absence de déclaration équivaut à `architecture hexagonal` avec les valeurs par défaut : `src: src` (répertoire `src/`), couches `infrastructure/`, `application/` et `domain/`, et aucune racine `shared` (aucun import entre contextes autorisé).
- `architecture permissive` désactive uniquement les contrôles de couches, de contextes et de frontières de `wiring`; le typage strict, les règles de visibilité et toutes les autres vérifications restent actifs.
- L'architecture hexagonale est stricte : toute dépendance utilisateur que le compilateur ne peut pas classer doit produire un diagnostic plutôt que d'être acceptée par défaut.
- Ce défaut strict peut rendre non compilables des projets Ocara existants qui ne suivent pas une arborescence hexagonale. Ils devront soit adopter ou adapter cette structure, soit déclarer `architecture permissive` pour conserver leur organisation actuelle.
- Les builtins `ocara.*` ne sont pas concernés par les règles de couches.
- Les vérifications architecturales n'introduisent aucun coût à l'exécution : elles sont faites à la compilation.
- **Tests unitaires** : un fichier de test (`ocaraunit`) est un fichier d'entrée comme un autre. L'architecture s'y applique de la même façon : son bloc `architecture`, sinon le défaut hexagonal. Un test qui importe le code d'un projet hexagonal est donc soumis aux mêmes règles de couches, de contextes et de `wiring`. Un test qui ne suit pas cette structure déclare `architecture permissive`.

### Migration des exemples et des tests

Le défaut hexagonal rend non conformes la plupart des exemples existants. Lors de l'implémentation, tous les exemples (`examples/*.oc`, `examples/advanced/*`, `examples/builtins`, `examples/generics`, `examples/mods`, `examples/from`, `examples/runtime`…) et les fichiers de tests unitaires (`examples/tests`, `examples/project/tests`) reçoivent `architecture permissive` dans leur fichier d'entrée. Seul `examples/advanced/mini_project_hexa` déclare `architecture hexagonal` (`src: context`) et sert de référence. `make regression` et les tests `ocaraunit` doivent rester verts après cette migration.

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
10. Un fichier de test `ocaraunit` est contrôlé comme un fichier d'entrée : une violation de couche y est rejetée, `architecture permissive` la lève.

## Décisions ouvertes avant implémentation

- Plusieurs blocs `architecture` ou un seul bloc global.
- Gestion de plusieurs projets/racines `src` dans un même graphe.
- Une racine partagée peut-elle contenir des couches, et comment les reconnaître lorsqu'elle est nommée `context.shared` ?
- Traitement des outils, fichiers d'entrée hors contexte et bibliothèques utilisateur externes.
- La validation porte-t-elle uniquement sur les imports directs (recommandé pour des diagnostics lisibles) ou aussi sur une fermeture transitive, sachant qu'un import direct constitue déjà le graphe de dépendances visible au compilateur ?
- Diagnostic ou erreur fatale lorsque la source ne peut pas être classée en mode strict.

## Outillage : extension VS Code

Les instructions d'architecture sont des instructions bas niveau du langage, comme `when` : elles partagent sa couleur dédiée (jaune `#FFCC00` en gras sur thème sombre, jaune orangé `#A86500` sur thème clair, voir [reflexion-declarations-conditionnelles-when.md](reflexion-declarations-conditionnelles-when.md)).

- **Coloration** : le mot-clé `architecture`, son mode (`hexagonal`, `permissive`) et les propriétés du bloc (`src`, `layers`, `shared`) prennent cette couleur.
- **Couches dans les namespaces et les imports** : dans `namespace context.home.app.usecase` et `import context.car.infra.db.CarRepository`, le segment de couche (`app`, `infra`, `domain`… selon les alias déclarés) est coloré de la même façon, pour identifier la couche d'un coup d'œil. La détection suit la configuration effective du programme (bloc `architecture` du fichier d'entrée, sinon valeurs par défaut) : jetons sémantiques du serveur de langage, et non une règle TextMate figée, puisque les alias sont propres à chaque projet.
- **Détection des couches par l'extension** : le serveur de langage expose, pour chaque fichier, son contexte et sa couche (ou « hors `src` ») ; l'encart Workflow les affiche (imports et structure annotés de leur couche).
- **Graphe d'appels** : les boîtes (classes, structs, interfaces, fonctions…) sont encapsulées dans un cadre jaune par couche — `domain`, `application`, `infrastructure` — lui-même regroupé par contexte ; les éléments hors `src` (configs, runtimes) restent hors cadre. L'export Markdown (Mermaid) reproduit ces cadres en `subgraph` imbriqués.

## Priorité / Complexité

**Priorité Moyenne** — structure hexagonale stricte par défaut, avec opt-out explicite via `architecture permissive`; apporte une vérification architecturale aux applications métier tout en préservant un chemin sans ces contraintes pour les autres programmes. **Complexité estimée : Structurelle** — configuration de projet, classification des sources, intégration au graphe d'import, diagnostics et contraintes spécifiques à `wiring`; la résolution actuelle s'appuie sur namespaces et chemins, et nécessite une vérification centralisée pour éviter des règles divergentes entre le chargement normal des imports et le pré-scan de `wiring`.

## Fichiers et surfaces concernés

À confirmer lors de l'implémentation : grammaire et AST du programme (`src/parsing/`), chargement/résolution des imports (`src/main.rs`, `src/core/interface_wiring.rs`), validations sémantiques et diagnostics (`src/sema/`, `src/core/`), tests du parser/imports/`wiring`, documentation canonique (`docs/EBNF.md`, §31 compris), ainsi que l'exemple `examples/advanced/mini_project_hexa` et sa commande de build.
