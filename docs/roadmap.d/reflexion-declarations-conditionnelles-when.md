# Réflexion : déclarations conditionnelles `when`

Statut : **non tranché** — idée proposée le 2026-10-08.

## Idée

Conditionner une déclaration (import, classe, interface, struct, générique, enum,
module, fonction, méthode, propriété, variable) à une condition connue à
la compilation. Plusieurs variantes d'une même déclaration peuvent
coexister : une seule est retenue pour un build donné.

```ocara
import ocara.System

class Bidule {

    when System::OS is 'windows'
    public method truc(): void {
        return
    }

    when System::OS is not 'windows'
    when System::BUILD is 'debug'
    public method truc(): int {
        return 1
    }

    when System::OS is not 'windows' and System::BUILD is 'release'
    public method truc(): int {
        return 1
    }
}
```

```ocara
import ocara.System

when System::OS is 'windows'
class Bidule {
    public method truc(): void {
        return
    }
}

when System::OS is not 'windows'
class Bidule {
    when System::OS is 'linux'
    public property ret:int = 1
    when System::OS is 'android'
    public property ret:int = 2

    when System::BUILD is 'debug'
    public method truc(): map<int, string> {
        return {self.ret: 'debuggage ' + System::OS}
    }

    when System::BUILD is 'release'
    public method truc(): int {
        return self.ret
    }
}
```

Lecture : « quand l'OS n'est pas Windows et que le build est release,
j'utilise la méthode en dessous ».

## Points à trancher

- **Mot-clé** : `when` (se lit comme une phrase, préféré) ou `tag` (évoque
  une étiquette plutôt qu'une condition).
- **Opérateurs** : `is`/`is not` propres à `when`, ou les comparaisons du
  langage (`equal`/`not equal`). `is` existe déjà pour le test de type
  (`x is string`) : à vérifier qu'il n'y a pas d'ambiguïté de lecture.
  - **is** à une lecture intuitive et explicite dans la ligné de (`x is string`) donc pas d'ambiguité
  - liste des condition disponible: 
    - `is`, `is not`, `is greater`, `is smaller` , `is greater or equal`, `is smaller or equal`
- **Plusieurs `when`** sur une déclaration : conjonction est équivalent à
  `and`
- **Constantes de build disponibles** : `System::OS`, `System::BUILD`, cible
  (`System::ARCH`, Android…) ; valeurs fixées par le compilateur (`--target`,
  mode debug/release) — toute autre expression refusée.
  - dans l'absolu on part sur les valeurs fixé par le compilateur
  - **Écarté (2026-10-08)** : conditions sur des variables de session
    (`HTTPServerSession`). Une valeur connue seulement à l'exécution ne dit
    pas quel code existe mais quel code s'exécute — rôle d'un `if`/`match`
    explicite ; elle rendrait l'appel ambigu à la lecture et casserait la
    vérification statique par variante (option A).
- **Variantes** : deux variantes retenues en même temps pour un build, ou
  aucune alors que la déclaration est utilisée → erreur de compilation.
  Les signatures peuvent-elles différer (`void` d'un côté, `int` de
  l'autre, comme dans l'exemple) ?
  - le processus de choix de la variante ce fait à la compilation. Les condition d'usage permette au compilateur de savoir quel variante doit être utilisé
  - **Tranché (option A)** : les signatures peuvent différer. La sema vérifie le
    programme une fois par combinaison des constantes réellement utilisées
    (ex. debug et release) ; un appel incompatible avec une variante doit
    lui-même être conditionné par `when`.
- **Outillage** : la variante écartée doit-elle quand même être vérifiée
  par la sema (sinon une erreur n'apparaît qu'en compilant pour l'autre
  cible) ? Coloration et survol dans l'extension VS Code.
  - l'extension doit prendre en compte les deux variantes. Car quand on dev on est sur un environnement précis, mais on veux que le code des autre condition d'usage soit aussi checker. Il faut savoir qu'une tache est prévus pour créer un serveur LSP au compilateur pour faire disparaitre le système heurestic de l'extension et augmenter sa precision et perf

## Coloration (exigence, 2026-10-08)

Une condition d'usage doit se repérer au premier regard : le mot-clé `when`
et ses opérateurs (`is`, `is not`, `is greater`, `is smaller`,
`is greater or equal`, `is smaller or equal`, `and`) partagent **une même
couleur**, distincte de toutes les autres couleurs du code.

Mise en œuvre prévue dans l'extension VS Code
(`tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`) :

- une règle dédiée qui reconnaît une ligne `when …` en tête de déclaration
  (`meta.condition.when.ocara`), placée avant les règles génériques pour que
  `is`/`and` n'y reçoivent pas leur couleur habituelle ;
- un scope commun au mot-clé et aux opérateurs
  (`keyword.control.directive.when.ocara`) ; les constantes
  (`System::OS`) et littéraux gardent leur coloration propre ;
- une couleur imposée quel que soit le thème, via
  `contributes.configurationDefaults` → `editor.tokenColorCustomizations`
  (`textMateRules` sur ce scope), plutôt que de dépendre de la couleur que
  chaque thème donne aux directives ;
- avec le futur serveur LSP : un type de jeton sémantique dédié, même
  couleur, et grisage de la variante écartée pour la configuration active.

## Mise en œuvre si retenue

- Parseur : clauses `when` en tête de déclaration.
- Passe de sélection avant la sema : évalue les conditions avec les
  constantes de build, retire les variantes écartées, détecte doublons et
  absences.
