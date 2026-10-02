# ocaracs — styles de nommage configurables et indentation

## Constat

Les règles de nommage d'ocaracs sont des interrupteurs `true`/`false` avec un
style figé :

| Règle | Clé | Style imposé |
|---|---|---|
| R07 | `naming_class` | PascalCase (classes, structs, interfaces, modules, generics) |
| R08 | `naming_function` | camelCase (fonctions et méthodes) |
| R09 | `naming_const` | UPPER_SNAKE_CASE (constantes globales ET de classe) |
| R12 | `naming_variable` | snake_case (var/scoped/consumed, propriétés) |

R09 s'applique aussi aux `const` déclarées **dans le corps d'une fonction ou
d'une méthode**. Exemple : `const server:HTTPServer = use HTTPServer()` dans
`main()` lève « constante 'server' devrait être en UPPER_SNAKE_CASE ». Une
constante locale se lit pourtant comme une variable locale, et le corpus
existant l'écrit déjà ainsi (`examples/builtins/httpserver_static.oc`…).

## Proposition

Nouvelles clés `.ocaracs`. Chaque clé `*_is` accepte une seule valeur parmi
`snake_case|camelCase|PascalCase|UPPER_SNAKE_CASE|UPPERCASE`.

| Règle | Clé | Défaut | Rôle |
|---|---|---|---|
| R13 | `naming_const_embed = true` | `true` | Les `const` dans une fonction ou une méthode suivent leur propre style (R14), plus celui de R09 |
| R14 | `naming_const_embed_is` | camelCase (voir point à trancher) | Style des `const` locales |
| R15 | `naming_const_is` | `UPPER_SNAKE_CASE` | Style de R09 (constantes globales et de classe) |
| R16 | `naming_var_is` | `snake_case` | Style de R12 (variables et propriétés) |
| R17 | `naming_function_is` | `camelCase` | Style de R08 |
| R18 | `naming_class_is` | `PascalCase` | Style de R07 (classes, structs, interfaces, modules, generics) |
| R19 | `indentation_type` | `space` | `space` ou `tab` : type d'indentation imposé (R01 ne vérifie aujourd'hui que la cohérence au sein d'un fichier) |
| R20 | `indentation_gap` | `4` | Largeur d'un niveau d'indentation (R01 déduit aujourd'hui l'unité de la première ligne indentée) |

Les valeurs par défaut de R15 à R18 reproduisent exactement le comportement
actuel : aucune régression sur un `.ocaracs` existant. Les interrupteurs
R07/R08/R09/R12 restent les seuls à activer ou désactiver une règle ; les
clés `*_is` n'en changent que le style.

## Points à trancher

- **Style par défaut de R14** : la demande dit « camelCase comme les
  variables ». Or R12 impose `snake_case` aux variables, et le libellé
  proposé pour R13/R14 dit lui aussi « snake_case ». Choisir entre :
  - `snake_case` : cohérent avec R12 et `docs/conventions.md` ;
  - `camelCase` : comme écrit dans la demande.
  Option possible : par défaut, R14 suit `naming_var_is` (une constante
  locale se nomme comme une variable locale).
- **Sens de R13 à `false`** : les `const` locales retombent-elles sous R09
  (comportement actuel), ou ne sont-elles plus vérifiées du tout ?
- **Valeur invalide** (`naming_var_is = kebab`) : avertissement au chargement
  du `.ocaracs` et repli sur le défaut, ou erreur bloquante ?
- **`UPPERCASE` vs `UPPER_SNAKE_CASE`** : `UPPERCASE` interdit-il le `_`
  (`MAXRETRY`) ? Probablement oui, sinon les deux valeurs se confondent.
- **R19 et R20 face à R01** : R01 (cohérence) reste-t-il un interrupteur
  séparé, R19/R20 ne faisant que fixer le type et la largeur attendus ?
- **Messages de correction** : convertir le nom vers le style cible
  (`→ maxRetry`) comme R09/R12 le font déjà pour leur style unique.

## À mettre à jour

`tools/ocaracs/src/` (configuration, règles de nommage, R01),
`tools/ocaracs/README.md` (liste des règles et exemple de `.ocaracs`),
`docs/conventions.md` (constantes locales), `.ocaracs` du dépôt si les
défauts changent. L'extension VS Code n'a rien à changer : elle lance
ocaracs avec le `.ocaracs` le plus proche.

## Priorité / Complexité

Moyenne — **Légère** (circonscrit à ocaracs : configuration, conversions de
casse, tests).
