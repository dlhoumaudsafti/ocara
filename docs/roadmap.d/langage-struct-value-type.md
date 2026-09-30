# `struct` façon C — type agrégat, potentiellement à sémantique de valeur

## Demande (David)

```ocara
struct Point {
    public property x:int
    public property y:int
}
```

« Une struct peut être `extends` » :

```ocara
var dto:Struct<Point> = use Point(10, 20)
var area:float = dto.x + dto.y
```

## Syntaxe des champs — révisée (David, confirmée pertinente)

Pas de `property`/`public` obligatoires — un champ nu est **implicitement
public**, avec valeur par défaut optionnelle (même mécanisme que
[langage-property-initializer](langage-property-initializer.md), dont ce
ticket devient dépendant plutôt que de réinventer un second mécanisme
d'évaluation de valeur par défaut) :

```ocara
struct UserDTO {
    id:int|null
    name:string = 'John'
    email:string = 'john.doe@example.com'
    protected age:int = 0
}
```

- **`public` reste un mot-clé valide mais redondant**, jamais obligatoire —
  s'écrit exactement comme sur une `class` si on veut la symétrie
  (`public email:string`), mais un champ nu sans mot-clé est déjà public par
  défaut.
- **`protected` a un sens** uniquement via `extends` : un champ protégé d'un
  struct de base reste accessible à un struct dérivé, inaccessible à tout
  code externe — même rôle que pour `class`.
- **`private` est REJETÉ à la compilation**, pas silencieusement toléré
  sans effet. Raison de fond : `private`/`protected` sur une `class`
  protègent un INVARIANT — empêcher du code externe de mettre l'objet dans
  un état incohérent, la classe se chargeant elle-même de le maintenir via
  ses méthodes. Un `struct` est par nature un agrégat de données
  transparent, sans invariant à défendre : vouloir un champ `private`
  dessus est le signal qu'on veut en réalité une `class`, pas un `struct`.
  En faire une erreur de compilation explicite (pas juste « inutile »)
  renforce la distinction struct/class au lieu de la laisser s'éroder.
- Un champ sans valeur par défaut (`id:int|null`, `name:string`) est
  obligatoire à la construction — interagit directement avec
  [langage-named-arguments](langage-named-arguments.md) pour permettre de ne
  fournir que les champs obligatoires (plus ceux qu'on veut surcharger) sans
  respecter l'ordre de déclaration.

## État actuel (vérifié dans le code)

`struct` n'existe pas du tout aujourd'hui — pas de mot-clé réservé
(`src/parsing/lexer.d/tokenizer.d/keywords.rs`), pas de nœud AST. Le seul
type agrégat utilisateur est `class` (`ClassDecl`, `src/parsing/ast.d/classes.rs`),
et **toute instance de classe est une allocation sur le tas** — `docs/EBNF.md`
§9/§16 : « Toutes les allocations heap (string, array, map, objet,
fat-pointer)... », l'analyse d'échappement décide seulement QUAND elle est
libérée (fin de bloc si prouvée non-échappante, sinon fin de programme),
jamais SI elle vit sur la pile. Il n'existe aujourd'hui **aucun type agrégat
à sémantique de valeur** (copié par affectation, vie sur la pile, pas
d'identité/allocation propre) dans le langage.

## Question centrale à trancher avant tout — c'est elle qui détermine tout le reste

L'exemple utilise `use Point(10, 20)` — exactement la même syntaxe
d'instanciation qu'une `class` aujourd'hui, qui signifie TOUJOURS allocation
tas. Deux lectures possibles, avec des implications radicalement
différentes :

**(A) `struct` est une vraie sémantique de valeur** (comme en C/C++/Rust/Swift/C#) :
alloué sur la pile, copié à l'affectation/au passage en paramètre, pas
d'identité propre, pas de destructeur, pas d'analyse d'échappement
nécessaire (jamais de fuite possible par construction). C'est la raison
d'être habituelle d'un `struct` distinct d'une `class` dans un langage qui a
déjà cette dernière — sinon `struct` n'apporterait rien de nouveau que `class`
n'offre déjà. Mais alors `use Point(10, 20)` (mot-clé aujourd'hui réservé à
l'allocation tas) serait trompeur/incohérent — il faudrait soit une syntaxe
de construction dédiée sans `use` (`Point(10, 20)` directement ?), soit
redéfinir ce que `use` signifie pour un `struct`.

**(B) `struct` est une variante restreinte de `class`** (toujours allouée
sur le tas via `use`, mais avec des propriétés publiques par défaut / pas de
méthodes / plus proche d'un DTO) — dans ce cas c'est surtout une distinction
sémantique/documentaire pour le lecteur du code, pas un nouveau mécanisme
runtime. Plus simple à implémenter (essentiellement un alias de `class` avec
des contraintes de déclaration), mais n'apporte pas le bénéfice usuel d'un
`struct` (éviter l'allocation tas pour de petits agrégats de données).

**Ce choix conditionne tout : la représentation runtime (pile vs tas), le
besoin ou non d'analyse d'échappement/ownership, la sémantique de
`extends`, et si `Struct<T>` a un sens propre au système de types.**

## Sous-points liés (dépendent de la réponse ci-dessus)

- **`extends` sur un `struct`** — en C++, l'héritage sur un `struct` est
  mécaniquement identique à celui d'une `class` (elles ne diffèrent que par
  la visibilité par défaut). Mais si (A) est retenu (sémantique de valeur),
  l'héritage entre en tension avec la taille fixe d'un agrégat empilé : une
  sous-classe ajoutant des champs change la taille de l'agrégat, et un
  dispatch polymorphe à travers une référence de type de base nécessite soit
  un pointeur de vtable embarqué dans la valeur (le « slicing » devient un
  risque à la copie, comme en C++), soit d'interdire le dispatch dynamique
  pour un `struct` (résolution statique uniquement, pas de polymorphisme
  runtime) — à trancher explicitement.
- **Position de type : `Point` directement, PAS de wrapper `Struct<T>`**
  (tranché — David). `use Point(10, 20)` produit directement une valeur de
  type `Point`, exactement comme pour une `class` — contrairement à
  `Resolvable<T>` (voir
  [langage-async-non-int-return-type-check](langage-async-non-int-return-type-check.md)),
  qui enveloppe un HANDLE n'étant pas encore la valeur réelle (justifiant le
  wrapper), un `struct` instancié EST directement la valeur, sans étape de
  déballage. Un wrapper n'apporterait aucune information et casserait la
  cohérence avec `class`. Convention universelle dans les langages ayant les
  deux concepts (C#, Swift, Rust, C++, Go) : le struct s'utilise nu comme
  type, la distinction struct/class se fait à la déclaration, jamais répétée
  à l'usage. Même conclusion appliquée à
  [langage-enum-cases-backing-methods](langage-enum-cases-backing-methods.md)
  (`OrderStatus` directement, pas `Enum<OrderStatus>`).
- Construction avec arguments nommés attendue dès le départ :
  `use Point(x: 10, y: 20)` — voir
  [langage-named-arguments](langage-named-arguments.md).
- **Si (A) est retenu** : interaction avec `array<T>`/`map<K,V>` (un
  tableau de `struct` serait-il un tableau de valeurs contiguës plutôt que
  de pointeurs — un vrai changement de représentation mémoire pour ces
  conteneurs) et avec le passage en paramètre de fonction (copie implicite à
  chaque appel, contrairement à une `class` passée par référence).

## Priorité / Complexité

**Haute** (demandé explicitement) — **Massive si (A)** (nouvelle
représentation runtime à sémantique de valeur, absente aujourd'hui à 100 % —
touche le lowering, l'ownership, les conteneurs génériques) / **Structurel
si (B)** (essentiellement une variante déclarative de `class`). La
complexité réelle ne peut être évaluée avant d'avoir tranché la question
centrale ci-dessus.

## Fichiers clés

`src/parsing/lexer.d/tokenizer.d/keywords.rs` (nouveau mot-clé),
`src/parsing/ast.d/classes.rs` (`ClassDecl`, point de comparaison),
`src/parsing/parser.d/declarations.rs`, `src/sema/typecheck.rs`,
`src/lower/builder.d/` (représentation runtime si sémantique de valeur),
`docs/EBNF.md` (§16 Classes, point de comparaison).
