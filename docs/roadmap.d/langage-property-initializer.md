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

## Mise en œuvre

**Désucrage au parsing** (`src/parsing/parser.d/property_init.rs`) : chaque
initialiseur devient `self.nom = expr` en tête du corps de `init()`, dans
l'ordre de déclaration ; un `init()` sans paramètre est synthétisé s'il
n'existe pas (`ClassDecl.implicit_init`). Toute la suite (alias, `wiring`,
sema, analyse des ressources/de l'échappement, lowering) voit une
affectation ordinaire — aucun traitement spécial ailleurs.

- **Héritage** : un `init()` synthétisé dans une classe dont un ancêtre a un
  constructeur reçoit ses paramètres et commence par `parent::init(...)`
  (`core::property_init`, après la fusion des imports) — le parent est
  construit d'abord. Un `init()` écrit doit, comme avant, appeler
  `parent::init(...)` lui-même.
- **`self`/`parent` interdits** (E56, au parsing, closures comprises) ;
  initialiseur sur une `property` de `module` refusé.
- **Type** : découvert en route — la sema ne vérifiait JAMAIS le type d'une
  valeur affectée à un champ (`self.x = "texte"` pour `x:int` passait,
  initialiseur ou non) ; c'est désormais un `TypeMismatch`. Aucun exemple
  existant n'en dépendait.
- **`init()` optionnel** : l'était déjà (une classe sans `init` s'instancie).

Tests : `src/core/tests_property_init.rs`,
`examples/tests/72_property_initializerTest.oc`. Documenté dans
`docs/EBNF.md` §16.3 (+ §31), `docs/diagnostics.md` (E56).

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
