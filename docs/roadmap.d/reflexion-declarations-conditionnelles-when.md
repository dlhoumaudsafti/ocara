# Réflexion : déclarations conditionnelles `when`

Statut : **non tranché** — idée proposée le 2026-10-08.

## Idée

Conditionner une déclaration (import, classe, interface, struct, générique, enum,
module, fonction, méthode, propriété, variable, runtime, return) ou une instruction d'un
corps de fonction/méthode à une condition connue à
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
    when default
    public property ret:string = "3"

    when System::BUILD is 'debug'
    public method truc(): map<int, string> {
        when System::OS is 'linux' or System::OS is 'android'
        return {self.ret: 'debuggage ' + System::OS}
        when default
        return {self.ret.toInt(): 'debuggage ' + System::OS}
    }

    when System::BUILD is 'release'
    public method truc(): int {
        when System::OS is 'linux' or System::OS is 'android'
        return self.ret
        when default
        return self.ret.toInt()
    }
}
```

Lecture : « quand l'OS n'est pas Windows et que le build est release,
j'utilise la méthode en dessous ».

### Runtime conditionnels

Un `runtime` se conditionne comme n'importe quelle autre déclaration :

```ocara
runtime core.init is init
when System::OS is not 'android'
runtime core.main is main
runtime core.mainAndroid is main
runtime core.error is error
runtime core.exit is exit
```

Ici, `core.main` est retenu comme bloc `main` hors Android ; sur Android, sa
clause est fausse et la variante sans clause, `core.mainAndroid`, prend le
relais (voir « Priorité entre variantes »).

### Priorité entre variantes (règle, 2026-10-08)

Les variantes d'une même déclaration (même genre et même nom, ou même bloc
cible pour un `runtime … is <bloc>`) sont examinées **dans l'ordre du
source** :

1. **La première variante dont la clause `when` est vraie est retenue** ;
   les variantes suivantes sont ignorées, même si leur clause est vraie
   aussi.
2. **Une variante sans clause `when` est le « sinon »** : elle n'est retenue
   que si aucune variante conditionnée n'est vraie, **quelle que soit sa
   place** dans le source (examinée en dernier).
3. **`when default`** désigne explicitement cette variante « sinon » :
   même comportement qu'une variante sans clause, écrit pour la lisibilité
   (convention recommandée dès qu'il existe d'autres variantes).

```ocara
when System::OS is 'windows'
function home(): string { return 'C:\\Users' }

when System::OS is 'linux'
function home(): string { return '/home' }

when default
function home(): string { return '/' }
```

Conséquences :

- Deux variantes conditionnées vraies en même temps ne sont plus une
  erreur : la première l'emporte. Un avertissement signale une variante
  **jamais retenue**, quelle que soit la cible (masquée par une variante
  précédente dont la clause est toujours vraie).
- Deux variantes « sinon » (sans clause ou `when default`) pour une même
  déclaration : erreur de compilation (laquelle retenir ?).
- **Variante absente** : aucune clause vraie et pas de « sinon » — la
  déclaration n'existe simplement pas pour cette cible. C'est un cas normal
  (ex. `ocara.Tauri` importé partout sauf sur Android) : **ni erreur ni
  avertissement**. Seul le code compilé pour cette cible qui l'utilise
  quand même est en erreur, comme pour n'importe quel symbole inconnu
  (`undefined symbol`) — d'où la règle de l'option A : ce code porte une
  clause compatible.
- `when default` est réservé aux déclarations qui ont d'autres variantes ;
  seul, il est sans effet (avertissement).

### Instructions conditionnelles

Dans un corps de fonction ou de méthode, une clause `when` conditionne
l'**instruction qui suit** (exemple `truc()` ci-dessus). Les instructions
conditionnées **consécutives** forment un groupe de variantes, sélectionné
avec la même règle de priorité que les déclarations (première clause vraie,
`when default` en dernier). Une instruction sans clause termine le groupe :
au sein d'un corps, le « sinon » s'écrit donc toujours `when default`.

### Placement de la clause (convention, 2026-10-08)

Par convention, une clause `when` est **collée** à la variante qu'elle
conditionne : aucune ligne blanche entre la (ou les) ligne(s) `when` et la
déclaration.

```ocara
when System::OS is 'linux'
function home(): string { return '/home' }
```

- **Compilateur** : les lignes blanches (et commentaires) entre la clause et
  la déclaration sont acceptées sans erreur ; la clause s'applique à la
  déclaration qui suit.
- **ocaracs** : avertissement de style si une ou plusieurs lignes blanches
  séparent une clause `when` de sa déclaration ; rien n'est modifié
  automatiquement, la correction (lignes blanches retirées) se fait à la
  demande avec `ocaracs --fix`.

### Imports conditionnels

Un `import` se conditionne comme n'importe quelle autre déclaration :

```ocara
import ocara.System

when System::OS is not 'android'
import ocara.Tauri

when System::OS is not 'android'
function openWindow(): void {
    var ui:Tauri = use Tauri({"title": "App", "url": "http://localhost:8080"})
    ui.run()
}
```

- Sur Android, `ocara.Tauri` n'est pas importé : ses symboles n'existent pas
  et son runtime (`runtime_tauri`) n'est pas lié.
- Conséquence de l'option A : tout code qui utilise un module importé sous
  `when` doit porter une clause compatible ; sinon la sema le signale
  (symbole inconnu) dans la variante où l'import est écarté.
- La sélection des imports a lieu dans la même passe que les autres
  variantes, avant la résolution des imports.

## Points à trancher

- **Mot-clé** : `when` (se lit comme une phrase, préféré)
- **Opérateurs** : `is`/`is not` propres à `when`, ou les comparaisons du
  langage (`equal`/`not equal`). `is` existe déjà pour le test de type
  (`x is string`) : à vérifier qu'il n'y a pas d'ambiguïté de lecture.
  - **is** à une lecture intuitive et explicite dans la ligné de (`x is string`) donc pas d'ambiguité
  - liste des condition disponible: 
    - `is`, `is not`, `is greater`, `is smaller` , `is greater or equal`, `is smaller or equal`
  - combinaison sur une même ligne : `and`, `or` (ex. `when System::OS is 'linux' or System::OS is 'android'`) ;
    priorité usuelle (`and` avant `or`), parenthèses autorisées
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
- **Variantes** : sélection par ordre de priorité (voir « Priorité entre
  variantes ») ; une déclaration sans variante retenue n'existe pas pour
  la cible (aucun diagnostic), seul son usage par du code compilé est en
  erreur. Les signatures peuvent-elles différer (`void` d'un côté, `int` de
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
`is greater or equal`, `is smaller or equal`, `and`, `or`, `default`) partagent **une même
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

- Parseur : clauses `when` (et `when default`) en tête de déclaration, y compris `import` et `runtime` ; lignes blanches tolérées entre la clause et la déclaration.
- ocaracs : règle « clause `when` séparée de sa déclaration par une ligne blanche » (avertissement ; corrigeable par `ocaracs --fix`).
- Passe de sélection avant la sema : regroupe les variantes de chaque
  déclaration, évalue leurs clauses dans l'ordre du source avec les
  constantes de build, retient la première vraie (sinon la variante sans
  clause ou `when default`), retire les autres ; signale les « sinon » en
  double et les variantes jamais retenues ; une déclaration sans variante
  retenue est simplement retirée (aucun diagnostic).
