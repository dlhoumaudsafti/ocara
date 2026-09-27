# Nouvelle instruction `wiring` dans une interface — liaison interface↔implémentation à la compilation

## Proposition

Permettre à une `interface` de déclarer, en son sein, une ou plusieurs
classes concrètes qui la servent — pour que le code consommateur ne dépende
JAMAIS d'une implémentation concrète (infra), seulement du contrat (domain).
Objectif explicite : renforcer le respect de l'architecture hexagonale déjà
en place dans `mini_project_hexa` (voir
[stdlib-httpserver-request-object](stdlib-httpserver-request-object.md) et
les tickets HTTPServer précédents, qui ont servi de terrain à cette
architecture) — le service applicatif ne doit importer que le port, jamais
l'adaptateur.

**Mécanisme unifié (généralisé après discussion — ne se limite pas aux
méthodes statiques)** : importer une interface avec un alias qui correspond
au nom simple d'un de ses `wiring` fait de cet alias un **stand-in
transparent pour la classe concrète visée**, utilisable PARTOUT où on
écrirait normalement le nom de cette classe — annotation de type,
construction (`use X(...)`), appel statique (`X::méthode()`) — après quoi
les appels d'instance (`x.méthode()`) suivent normalement, puisque `x` est
alors typé comme la classe concrète elle-même. Ce n'est pas une dispatch
spéciale limitée au statique : c'est un alias qui redirige vers la classe
concrète à tous les points d'usage, exactement comme si le fichier avait
directement importé cette classe sous ce nom.

Exemple avec une méthode statique (`context.home`), **plusieurs
implémentations wired** :

```ocara
// context/home/domain/contract/CarSummaryInterface.oc
interface CarSummaryInterface {
    wiring context.home.infra.db.CarSummaryRepository
    wiring context.home.infra.db.SuperCarSummaryRepository
    public static method all(): array<CarSummaryEntity>
}
```

```ocara
// context/home/infra/db/CarSummaryRepository.oc
import context.home.domain.contract.CarSummaryInterface
class CarSummaryRepository implement CarSummaryInterface {
    private static method fromRow(row:map<string, mixed>): CarSummaryEntity { ... }
    public static method all(): array<CarSummaryEntity> { ... }
}
// (SuperCarSummaryRepository, ailleurs, implémente la même interface de la même façon)
```

Trois façons d'appeler, selon l'import dans `context/home/app/services/ExempleService.oc` :

```ocara
import context.home.domain.contract.CarSummaryInterface
class ExempleService {
    init() { CarSummaryInterface::all() } // pas d'alias → 1er wiring déclaré → CarSummaryRepository
}
```
```ocara
import context.home.domain.contract.CarSummaryInterface as CarSummaryRepository
class ExempleService {
    init() { CarSummaryRepository::all() } // alias = nom simple du 1er wiring → CarSummaryRepository
}
```
```ocara
import context.home.domain.contract.CarSummaryInterface as SuperCarSummaryRepository
class ExempleService {
    init() { SuperCarSummaryRepository::all() } // alias = nom simple du 2e wiring → SuperCarSummaryRepository
}
```

Même mécanisme avec une méthode d'**instance** (l'interface déclare
`public method all(...)`, pas `static`) — l'alias sert aussi de type et de
cible de construction :

```ocara
// context/home/domain/contract/CarSummaryInterface.oc
interface CarSummaryInterface {
    wiring context.home.infra.db.CarSummaryRepository
    wiring context.home.infra.db.SuperCarSummaryRepository
    public method all(): array<CarSummaryEntity>
}
```
```ocara
// context/home/app/services/ExempleService.oc
import context.home.domain.contract.CarSummaryInterface as SuperCarSummaryRepository
class ExempleService {
    public property repository:SuperCarSummaryRepository
    init() {
        self.repository = use SuperCarSummaryRepository()
        self.repository.all()
    }
}
```
Ici, `SuperCarSummaryRepository` (l'alias) est utilisé comme type de
`property`, comme cible de `use ...()`, puis `self.repository` se comporte
comme une instance ordinaire de la classe concrète — aucune règle
supplémentaire nécessaire pour l'appel d'instance `.all()`, il découle
simplement du type réel de `self.repository` une fois l'alias substitué.

## Règles de résolution, décidées

- Une interface peut avoir **plusieurs** `wiring` (pas une seule liaison fixe).
- Import **sans alias** (`import Interface`) → toute position qui nécessite
  une classe concrète (construction, appel statique) résout vers le
  **premier `wiring` déclaré, dans l'ordre textuel** du corps de l'interface
  (haut en bas — pas l'ordre d'import, pas l'ordre alphabétique).
- Import **avec alias** (`import Interface as X`) → `X` doit correspondre
  **exactement** (comparaison sensible à la casse) au **nom simple** (dernier
  segment du chemin qualifié) d'UN des `wiring` déclarés dans cette
  interface ; `X` devient alors un alias transparent vers CETTE classe
  précise, dans toutes les positions où `X` est utilisé (type, `use X(...)`,
  `X::méthode()`). Un alias qui ne correspond à aucun `wiring` de
  l'interface importée est une erreur de compilation.
- Cette résolution est **scopée à l'import concerné** : l'alias n'est
  jamais recherché parmi les `wiring` d'une AUTRE interface, même si
  plusieurs interfaces sont importées dans le même fichier.
- **Le nom RÉEL (non aliasé) de l'interface, utilisé comme simple annotation
  de type** (`var x:CarSummaryInterface = ...`, `param:CarSummaryInterface`)
  — **reste le type abstrait**, avec le polymorphisme classique déjà permis
  par `implement` aujourd'hui : peut recevoir n'importe quelle instance
  d'une classe qui implémente l'interface, wired ou non. La substitution
  décrite ci-dessus ne s'applique QUE dans les positions qui n'ont
  aujourd'hui aucun sens pour une interface nue (construction `use X(...)`,
  appel statique `X::méthode()`) — jamais à une annotation de type ordinaire
  sur le nom réel de l'interface. Autrement dit : `wiring` rend meaningful
  des positions qui étaient des erreurs de compilation avant ce ticket ; il
  ne change jamais le sens de ce qui compilait déjà.
- Une fois la substitution appliquée dans une position donnée (ex. le type
  d'une `property`), tout le reste suit normalement les règles déjà
  existantes du langage — aucune nouvelle règle nécessaire pour les appels
  d'instance eux-mêmes.

## Autres points déjà tranchés

- **Conformité obligatoire** : la classe visée par un `wiring` doit
  explicitement écrire `implement CarSummaryInterface` — le compilateur
  vérifie la compatibilité de signature avant d'accepter le `wiring`.
- **Import implicite** : `wiring` agit comme un import implicite de CHAQUE
  classe visée — aujourd'hui, un fichier n'est chargé dans le programme
  compilé que s'il est atteint par la chaîne des `import` depuis le fichier
  principal (boucle `imports_to_process`, `src/main.rs`) ; si aucun fichier
  consommateur n'importe directement `CarSummaryRepository`/
  `SuperCarSummaryRepository`, ces classes ne seraient jamais chargées sans
  ce comportement implicite.
- **`wiring` uniquement dans une `interface`** : l'utiliser ailleurs (dans une
  `class`, un `module`, au niveau fichier...) est une erreur de compilation.
  Aucun risque de collision de mot réservé à vérifier (`wiring` n'existe pas
  du tout aujourd'hui dans le langage).
- **Diagnostics obligatoires** :
  - Construction/appel statique sur une interface **sans aucun `wiring`
    déclaré** → erreur de compilation (l'interface reste utilisable
    normalement en annotation de type/polymorphisme d'instance classique,
    juste pas en construction/appel statique direct).
  - `wiring` vers une classe introuvable → erreur de compilation.
  - `wiring` vers une classe qui n'écrit pas `implement CetteInterface` (ou
    dont les signatures ne correspondent pas) → erreur de compilation.
  - Alias qui ne correspond à aucun `wiring` de l'interface importée →
    erreur de compilation.
  - **Deux `wiring` de la MÊME interface partageant le même nom simple**
    (ex. `wiring context.a.infra.CarSummaryRepository` et
    `wiring context.b.infra.CarSummaryRepository` — chemins différents, nom
    simple identique `CarSummaryRepository`) → erreur de compilation à la
    déclaration de l'interface elle-même (pas seulement au moment d'un
    import qui s'en servirait), signalant le doublon d'alias entre les deux
    `wiring` fautifs — jamais un choix silencieux par ordre de déclaration.
  - (Nouveaux codes E-xx à documenter dans `docs/diagnostics.md`.)

## Ce qui reste ouvert

- **Portée du magasin d'injection** : cette conception reste une liaison
  100% statique/compile-time (pas de conteneur DI au sens runtime, pas de
  changement de `wiring` selon l'environnement/la cible de build) — à
  documenter clairement comme limitation assumée d'une v1, pas un oubli.

## Priorité / Complexité

**Massif.** Ce n'est pas une classe builtin de plus, et la généralisation
« l'alias vaut substitution partout, pas seulement en appel statique »
touche encore plus de terrain que la version initiale de ce ticket : la
grammaire (`docs/EBNF.md`, nouveau mot-clé et nouvelle règle dans le corps
d'une `interface`, répétable), le parser, la résolution d'imports (import
implicite via `wiring`, potentiellement plusieurs classes par interface), la
couche sema (vérification de conformité par wiring, résolution alias→wiring
appliquée à CHAQUE position où un identifiant peut désigner un type/une
classe — annotation de type, `use`, appel statique — pas seulement à la
résolution d'appel), et la couche lower/codegen (une fois la substitution
faite, tout le reste doit se comporter EXACTEMENT comme si la classe
concrète avait été importée directement, sans traitement spécial résiduel).
Probablement le chantier langage le plus profond de cette roadmap à ce
jour — à ne pas sous-estimer, et à re-scoper précisément une fois attaqué
(comme [runtime-httpserver-race-condition](runtime-httpserver-race-condition.md)
l'a montré en sens inverse : une estimation initiale peut se réviser une
fois le problème correctement recadré, dans un sens ou dans l'autre).

## Fichiers clés

`docs/EBNF.md` (nouvelle règle grammaticale pour `interface`, répétable),
`src/parsing/` (lexer : nouveau mot-clé ; parser : nouvelle règle dans le
corps d'une interface, plusieurs occurrences autorisées), `src/main.rs`
(résolution d'imports : chaque `wiring` d'une interface chargée comme import
implicite ; résolution de l'alias vers la classe concrète AVANT que le reste
du pipeline ne voie la différence), `src/sema/` (vérification de conformité
classe↔interface par wiring, résolution alias→wiring, nouveaux
diagnostics), `docs/diagnostics.md`, `tools/highlight/` (coloration du
nouveau mot-clé).
