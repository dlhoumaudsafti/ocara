# Ocara — Conventions de nommage

> Convention officielle du projet. Elle répond au ticket [qualite-convention-nommage-methodes](roadmap.d/qualite-convention-nommage-methodes.md) : jusqu'ici, ni le compilateur ni `docs/EBNF.md` n'imposaient de style, et le corpus d'exemples mélangeait `snake_case`/`camelCase` selon l'ancienneté du fichier. Ce document tranche.

---

## Résumé

| Catégorie | Convention | Exemple |
|---|---|---|
| Variable (locale, paramètre, propriété de classe) | `snake_case` | `var user_count:int`, `private property click_count:int` |
| Constante (`const`, y compris constante de classe) | `MAJUSCULES_SOUS_TIRET` | `const MAX_SCORE:int = 100`, `public const NOT_FOUND:int = 404` |
| Fonction / méthode | `camelCase` | `function computeTotal(...)`, `public method tryLock(): bool` |
| Classe / interface / module / generic | `PascalCase` | `class HttpStatus`, `interface Drawable`, `module Clickable`, `generic Stack<T>` |

Ces quatre règles couvrent tout identifiant déclaré en Ocara. En cas de doute sur une catégorie non listée ici, se rapprocher de la catégorie la plus proche par rôle (ex. un paramètre de closure `nameless` est une variable → `snake_case`) plutôt que d'introduire un cinquième style.

---

## Détail

### Variables — `snake_case`

S'applique à toute variable `var`/`scoped`/`consumed` locale, tout paramètre de fonction/méthode/closure, et toute propriété de classe (`property`), quelle que soit sa visibilité (`public`/`protected`/`private`).

```ocara
var user_count:int = 0
scoped total_price:float = 0.0

function greet(first_name:string, is_formal:bool): string { ... }

class Car {
    private property purchase_price:float
    public  property estimated_sale_price:float
}
```

### Constantes — `MAJUSCULES_SOUS_TIRET`

S'applique à `const` (global ou local) et aux constantes de classe (`public`/`protected`/`private const`). C'est déjà la convention majoritairement suivie dans le corpus existant (`Math::PI`, `HttpStatus::NOT_FOUND`, `Color::RED`) — ce document la rend officielle plutôt que simplement dominante.

```ocara
const MAX_RETRY:int = 3

class HttpStatus {
    public const NOT_FOUND:int = 404
    public const INTERNAL_ERROR:int = 500
}
```

### Fonctions et méthodes — `camelCase`

S'applique aux fonctions libres, aux méthodes d'instance et statiques, publiques comme privées. C'est déjà la convention à 100 % des builtins du runtime (`strToInt`, `tryLock`, `renderCached`) — la nouveauté est de l'étendre explicitement au code utilisateur, où les exemples les plus anciens utilisaient encore `snake_case` (`is_adult`, `get_score`).

```ocara
function computeTotal(items:array<float>): float { ... }

class Timer {
    public method isDone(): bool { ... }
    private method resetInternal(): void { ... }
}
```

### Classes, interfaces, modules, generics — `PascalCase`

S'applique au nom de toute déclaration `class`, `interface`, `module` et `generic<T>`, ainsi qu'aux variantes d'`enum` (`enum HttpStatus { Ok, Created, NotFound }` — les membres sont des identifiants de type, pas des constantes au sens `const`, donc `PascalCase` et non `MAJUSCULES_SOUS_TIRET`). C'est déjà la convention à 100 % du corpus existant, sans exception relevée.

```ocara
class BankAccount { ... }
interface Drawable { ... }
module Clickable { ... }
generic Stack<T> { ... }
enum HttpStatus { Ok = 200, NotFound = 404 }
```

---

## Portée et suites

- Cette convention s'applique à **tout code écrit à partir de maintenant** (doc, exemples, stdlib). Elle n'entraîne **aucune réécriture rétroactive en masse** du corpus existant.
- `ocaracs` (l'analyseur de style du projet, voir [tools/ocaracs/README.md](../tools/ocaracs/README.md)) n'émet aujourd'hui aucun avertissement sur ces conventions — les faire porter par le linter reste un point ouvert, suivi dans [qualite-convention-nommage-methodes](roadmap.d/qualite-convention-nommage-methodes.md).
