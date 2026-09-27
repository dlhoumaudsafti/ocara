# Initialiseur inline sur une `property` — `property nom:Type = expr`

## Proposition

Aujourd'hui, une `property` ne peut porter aucune valeur à sa déclaration
(vérifié directement : `public property x:int = 5` échoue au parsing —
`PropertyDecl ::= Visibility "property" Identifier ":" Type`, aucune clause
de valeur par défaut, voir `docs/EBNF.md` §"Initialisation implicite des
`property`"). La seule façon d'assigner une valeur non-zéro est de l'écrire
explicitement dans `init()`, y compris pour une valeur totalement
indépendante de tout paramètre du constructeur :

```ocara
import context.home.domain.contract.CarSummaryInterface as SuperCarSummaryRepository
class ExempleService {
    public property repository:SuperCarSummaryRepository
    init() {
        self.repository = use SuperCarSummaryRepository()
    }
}
```

Proposé : autoriser une expression d'initialisation directement à la
déclaration, évaluée et assignée automatiquement avant le corps de `init()`
— pas besoin d'écrire l'affectation à la main quand la valeur ne dépend
d'aucun paramètre du constructeur :

```ocara
import context.home.domain.contract.CarSummaryInterface as SuperCarSummaryRepository
import ocara.System

class ExempleService {
    public property repository:SuperCarSummaryRepository = use SuperCarSummaryRepository()
    public property os:string = System::OS
    init() {
        self.repository.all()
    }
}
```

Complément naturel de [langage-interface-wiring](langage-interface-wiring.md)
(illustré par ce même exemple) mais utile indépendamment de ce ticket — un
initialiseur de property est une fonctionnalité orientée-objet standard,
absente d'Ocara aujourd'hui.

## Sémantique proposée

- Chaque expression d'initialisation est évaluée et assignée **avant le
  corps de `init()`**, dans l'**ordre de déclaration** des `property` (haut
  en bas dans le corps de la classe).
- Une `property` avec initialiseur et RÉ-assignée explicitement dans
  `init()` (ex. `self.repository = use AutreChose()`) — l'affectation de
  `init()` l'emporte simplement, comme toute affectation normale qui suit
  une autre : aucune règle nouvelle nécessaire, ça découle directement de
  l'ordre d'exécution ci-dessus.
- Une `property` SANS initialiseur garde le comportement actuel
  (auto-zéro : `null` pour un type référence, `0`/`0.0`/`false` pour un
  type primitif, si jamais assignée dans `init()`).

## Décidé — v1 restreinte : l'initialiseur ne peut pas référencer `self`

Pour éviter toute question d'ordre d'initialisation ENTRE properties (une
property déclarée après une autre qui la référence n'existe pas encore),
l'expression d'initialisation doit être **autonome** : littéral, appel de
fonction/méthode statique, construction (`use X(...)`), etc. — mais jamais
`self.<autre_champ>`. Utiliser `self` dans un initialiseur de property est
une erreur de compilation en v1. Assouplissement (ordre de déclaration
faisant foi, à la manière des champs de classe JS/TS) explicitement différé
à une itération future si un besoin réel apparaît — pas trainé dès le
départ pour garder ce premier chantier simple.

## Ce qu'il faut encore vérifier/trancher en implémentant

- **Interaction avec l'héritage** : si une classe parente a des `property`
  avec initialiseur et qu'une classe fille appelle `parent::init()`, les
  initialiseurs du parent doivent s'exécuter comme partie de
  `parent::init()` (avant le corps du `init()` du parent), puis ceux de la
  fille avant le corps du `init()` de la fille — comportement standard dans
  la plupart des langages OO, mais à vérifier explicitement contre le
  mécanisme d'héritage actuel d'Ocara (`extends`/`parent::init()`).
- **Analyse d'échappement/ressource** : un initialiseur qui construit une
  ressource native (ex. `property db:SQLite = SQLite::open(...)`) doit
  passer par la même analyse que si l'affectation avait été écrite dans
  `init()` — vérifier que le point d'assignation "virtuel" (avant le corps
  de `init()`) est bien vu par `src/sema/scope.rs`/`src/sema/escape.rs`
  (mêmes vérifications que pour une `property` de type ressource, voir
  [langage-destructeur-champ-ressource](langage-destructeur-champ-ressource.md),
  déjà clos).
- **`init()` devient-il optionnel ?** Question probablement hors périmètre
  de CE ticket mais adjacente : si toutes les `property` d'une classe ont un
  initialiseur et qu'aucune logique de constructeur supplémentaire n'est
  nécessaire, faut-il pouvoir omettre `init()` entièrement ? Aujourd'hui
  `init()` semble obligatoire (présent dans tous les exemples existants,
  même vide `init() {}`) — à confirmer, et à traiter comme un ticket séparé
  si retenu plutôt que d'élargir la portée de celui-ci.

## Priorité / Complexité

Complexité non finement évaluée avant implémentation — **Structurel** :
touche la grammaire (`docs/EBNF.md`, `PropertyDecl` gagne une clause de
valeur optionnelle), le parser, la couche sema (nouvelle vérification
"pas de `self` dans un initialiseur de property", ordre d'exécution
héritage-aware, intégration avec l'analyse de ressources existante), et le
lower/codegen (générer l'affectation "virtuelle" avant le corps de chaque
`init()`, chaînée correctement à travers `parent::init()`).

## Fichiers clés

`docs/EBNF.md` (`PropertyDecl`), `src/parsing/` (parser : clause de valeur
optionnelle sur `property`), `src/sema/` (rejet de `self` dans un
initialiseur, vérification ressource/échappement), `src/lower/builder.d/`
(construction du corps effectif de `init()`, ordre parent→enfant),
`docs/diagnostics.md` (nouveau code E-xx pour `self` interdit dans un
initialiseur).
