# Modificateurs incomplets sur une méthode d'`interface` — seules 2 combinaisons sur 12 acceptées

## Constat (les 12 combinaisons testées une par une, pas supposées)

Une méthode de **classe** ordinaire accepte déjà `public`/`private`/
`protected` combinés librement à `static` et `async` :

```ocara
class Foo {
    public async method a(): int { return 1 }
    private static method b(): int { return 2 }
    protected static async method c(): int { return 3 }
}
```

Demande explicite de l'utilisateur : la même liberté doit exister dans le
corps d'une `interface`, pour les **12 combinaisons** (3 visibilités ×
[rien, `static`, `async`, `static async`]). Vérifié une par une sur cette
version du compilateur — seules 2 des 12 passent :

| Modificateurs | Résultat |
|---|---|
| `public method` | **OK** |
| `public static method` | **OK** |
| `public async method` | échec — `expected Method, found Async` |
| `public static async method` | échec — `expected Method, found Async` |
| `protected method` | échec — `expected Method, found Protected` |
| `protected static method` | échec — `expected Method, found Protected` |
| `protected async method` | échec — `expected Method, found Protected` |
| `protected static async method` | échec — `expected Method, found Protected` |
| `private method` | échec — `expected Method, found Private` |
| `private static method` | échec — `expected Method, found Private` |
| `private async method` | échec — `expected Method, found Private` |
| `private static async method` | échec — `expected Method, found Private` |

Les deux qui passent (`public`, `public static`) ont été ajoutées très
récemment comme prérequis du chantier [langage-interface-wiring](langage-interface-wiring.md)
(ses propres exemples utilisaient déjà `public static method`, qui n'était
pas grammatical avant ce ticket) — jamais pensées comme une grammaire
complète à l'époque, juste le minimum nécessaire pour `wiring`.

## Portée demandée

Rendre le corps d'une `interface` grammaticalement symétrique à celui d'une
`class` pour ces modificateurs : les 12 combinaisons du tableau ci-dessus
doivent toutes parser. C'est prioritairement un travail de **grammaire**
(symétrie avec ce qui existe déjà pour `class`), pas une nouvelle sémantique
à concevoir de zéro — voir la note ci-dessous sur `private`/`protected`
pour la seule nuance qui reste à trancher en l'implémentant, mais elle ne
doit pas bloquer l'objectif principal (accepter la grammaire).

## Note pour l'implémentation — `private`/`protected` sans corps de méthode

Une interface ne déclare aujourd'hui que des signatures (aucun corps de
méthode par défaut). `async` a un sens immédiat et sans ambiguïté (cohérent
avec ce qui existe déjà pour les classes). `private`/`protected` sont moins
évidents à interpréter tant qu'une interface n'a aucun corps de méthode à
elle — mais l'utilisateur a explicitement demandé que la grammaire les
accepte quand même, donc les traiter au minimum comme des annotations
acceptées et cohérentes avec la déclaration `implements` (voir point
suivant), sans nécessairement leur inventer une sémantique d'exécution
avant qu'un besoin concret n'apparaisse (ex. méthodes par défaut, pas
encore une fonctionnalité de ce langage).

- **Conformité `implements`** : si une interface déclare
  `protected async method a(): int`, la classe qui `implements` doit-elle
  IMPÉRATIVEMENT reprendre exactement les mêmes modificateurs sur sa propre
  méthode (vérification de signature stricte incluant visibilité/`static`/
  `async`, pas seulement le nom/type de retour/paramètres) ? À vérifier si
  c'est déjà le comportement actuel pour `public`/`static` ou si c'est un
  trou de vérification séparé, à combler dans le même chantier si c'en est
  un.

## Priorité / Complexité

**Priorité Haute** — trou de cohérence du langage (une méthode de classe
accepte déjà les 12 combinaisons, une méthode d'interface n'en accepte que
2), demandé explicitement à ce niveau de priorité. Complexité probablement
Légère : la grammaire équivalente existe déjà et fonctionne pour `class`
(même mots-clés, même position syntaxique) — il s'agit d'aligner le corps
de `interface` dessus, pas d'inventer de nouveaux tokens/règles.

## Fichiers clés

`src/parsing/parser.d/declarations.rs` (grammaire du corps d'une
`interface` — c'est là que `public`/`static` ont été ajoutés pour
`wiring` ; comparer avec la grammaire déjà correcte du corps d'une `class`
pour les 12 combinaisons, probablement dans le même fichier ou un fichier
voisin), `docs/EBNF.md` (règle de l'interface, à re-synchroniser avec le
§31 comme d'habitude), `src/sema/` (si la vérification de conformité
`implements` stricte sur les modificateurs, ci-dessus, est retenue dans ce
même chantier).
