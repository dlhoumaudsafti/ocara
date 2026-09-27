# Nouvelle instruction `wiring` dans une interface — liaison interface↔implémentation à la compilation

## Proposition

Permettre à une `interface` de déclarer, en son sein, la classe concrète qui
la sert par défaut — pour que le code consommateur ne dépende JAMAIS de
l'implémentation concrète (infra), seulement du contrat (domain), tout en
gardant un appel statique simple. Objectif explicite : renforcer le respect
de l'architecture hexagonale déjà en place dans `mini_project_hexa` (voir
[stdlib-httpserver-request-object](stdlib-httpserver-request-object.md) et
les tickets HTTPServer précédents, qui ont servi de terrain à cette
architecture) — le service applicatif ne doit importer que le port, jamais
l'adaptateur.

Exemple donné (`context.home`) :

```ocara
// context/home/domain/contract/CarSummaryInterface.oc
interface CarSummaryInterface {
    wiring context.home.infra.db.CarSummaryRepository
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
```

```ocara
// context/home/app/services/ExempleService.oc
import context.home.domain.contract.CarSummaryInterface as CarSummaryRepository
class ExempleService {
    init() {
        CarSummaryRepository::all()
    }
}
```

`ExempleService` n'importe QUE l'interface (aliasée localement au nom de la
classe concrète pour la lisibilité au point d'appel) — jamais
`context.home.infra.db.CarSummaryRepository` directement. L'appel statique
`CarSummaryRepository::all()` (en réalité un appel sur l'interface, via son
alias local) doit être résolu par le compilateur vers l'implémentation
déclarée par `wiring`.

## Ce qu'il faut trancher avant d'implémenter — c'est la partie la plus lourde de ce ticket

- **L'alias est-il purement cosmétique ?** L'exemple aliase systématiquement
  l'interface au nom de la classe concrète (`as CarSummaryRepository`). Un
  appel `CarSummaryInterface::all()` (import sans alias) doit-il fonctionner
  IDENTIQUEMENT (dispatcher vers la classe wired) ? Probable, mais à
  confirmer explicitement — l'alias ne devrait changer que le nom local, pas
  la sémantique de résolution.
- **Plusieurs implémentations possibles** : `implement` (déjà existant)
  autorise plusieurs classes à implémenter la même interface (ex. un
  `SQLiteCarSummaryRepository` ET un `MySQLCarSummaryRepository`
  interchangeables). `wiring` tel que proposé ne permet qu'UNE seule classe
  liée, fixée une fois pour toutes dans le fichier de l'interface. Est-ce
  la portée voulue pour une v1 (liaison unique, statique, décidée une fois),
  ou faut-il déjà penser à un mécanisme de substitution (tests avec un faux
  / mock, choix par cible de build, etc.) ? Une v1 à liaison unique est plus
  simple et probablement suffisante pour commencer — mais le limiter
  clairement dans la doc pour ne pas laisser croire à un vrai conteneur
  d'injection de dépendances tant que ce n'est pas construit.
- **Vérification de conformité** : la classe wired doit-elle explicitement
  écrire `implement CarSummaryInterface` (comme dans l'exemple) pour que le
  compilateur vérifie la compatibilité de signature avant d'accepter le
  `wiring` ? Presque certainement oui — sans ça, `wiring` pointerait vers
  n'importe quelle classe sans aucune garantie que ses méthodes correspondent
  au contrat déclaré.
- **Résolution d'import implicite** : aujourd'hui, un fichier n'est chargé
  dans le programme compilé QUE s'il est atteint par la chaîne des `import`
  depuis le fichier principal (voir la boucle `imports_to_process` de
  `src/main.rs`). Si `ExempleService.oc` n'importe QUE l'interface (jamais
  `CarSummaryRepository` directement), `wiring` doit agir comme un IMPORT
  IMPLICITE de la classe visée — sinon cette classe ne serait jamais chargée
  ni liée, même si elle existe sur disque. C'est un changement dans le
  mécanisme de résolution d'imports lui-même, pas seulement dans le parsing
  des interfaces.
- **Erreurs à diagnostiquer** : appeler statiquement une méthode d'une
  interface SANS `wiring` déclaré (utilisation normale actuelle d'une
  interface, en polymorphisme d'instance) doit rester une erreur claire, pas
  un comportement silencieusement différent. Un `wiring` pointant vers une
  classe introuvable, ou qui n'implémente pas réellement l'interface,
  doivent aussi être des diagnostics explicites (nouveaux codes E-xx à
  documenter dans `docs/diagnostics.md`).
- **Portée du mot-clé `wiring`** : nouveau mot-clé réservé — vérifier qu'il
  ne collisionne avec aucun identifiant déjà utilisé dans le corpus existant
  avant de le réserver (même précaution que pour toute nouvelle syntaxe,
  voir [reflexion-syntaxe-for-range](reflexion-syntaxe-for-range.md) pour un
  exemple de cette vérification sur une proposition précédente).

## Priorité / Complexité

**Massif.** Ce n'est pas une classe builtin de plus : ça touche la
grammaire (`docs/EBNF.md`, nouveau mot-clé et nouvelle règle dans le corps
d'une `interface`), le parser, la résolution d'imports (import implicite via
`wiring`), la couche sema (vérification de conformité, nouveaux
diagnostics), et la résolution des appels statiques (retrouver la classe
liée derrière un identifiant qui référence en réalité une interface).
Probablement le chantier langage le plus profond de cette roadmap à ce
jour — à ne pas sous-estimer, et à re-scoper précisément (comme
[runtime-httpserver-race-condition](runtime-httpserver-race-condition.md)
l'a montré en sens inverse : une estimation initiale peut se réviser une
fois le problème correctement recadré, dans un sens ou dans l'autre).

## Fichiers clés

`docs/EBNF.md` (nouvelle règle grammaticale pour `interface`), `src/parsing/`
(lexer : nouveau mot-clé ; parser : nouvelle règle dans le corps d'une
interface), `src/main.rs` (résolution d'imports : `wiring` comme import
implicite), `src/sema/` (vérification de conformité classe↔interface,
nouveaux diagnostics), `src/lower/`/`src/codegen/` (résolution d'un appel
statique à travers un alias d'interface vers la classe liée),
`docs/diagnostics.md`, `tools/highlight/` (coloration du nouveau mot-clé).
