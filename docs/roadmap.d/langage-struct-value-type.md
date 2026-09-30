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

## Décisions (tranchées, implémentées)

- **(B) retenu** : variante déclarative de `class` — tas via `use`, passage
  par référence, nullable (`Dto|null`), gestion mémoire identique. Pas de
  sémantique de valeur (une vraie sémantique de valeur pourra venir plus
  tard sous un autre nom sans casser l'existant). Pas de wrapper `Struct<T>`.
- **Membres** : champs (nus ou `[public|protected] property`, valeur par
  défaut optionnelle) et constantes uniquement — méthode, `init`, `modules`,
  `implements` rejetés au parsing.
- **Constructeur généré** depuis les champs, parents d'abord, noms et
  défauts des champs — utilisable en positionnel et avec les arguments
  nommés. Les valeurs par défaut des champs passent par les défauts des
  paramètres de ce constructeur : aucune dépendance à
  [langage-property-initializer](langage-property-initializer.md), qui reste
  un ticket distinct pour `class`.
- **Héritage** : struct → struct uniquement (E52) ; champ hérité redéclaré
  rejeté (E53) ; `private` rejeté (E51).
- **Arité des constructeurs** désormais vérifiée en sema pour tout
  `use X(...)` utilisateur (classes comprises) — un argument manquant/en trop
  n'échouait jusqu'ici qu'au codegen.

Mise en œuvre : `src/parsing/parser.d/struct_decl.rs` (parsing + constructeur
des champs propres), `src/core/structs.rs` (E51-E53, champs hérités),
`ClassDecl.is_struct`. Documenté dans `docs/EBNF.md` §16.7.

La visibilité `protected` des champs (commune aux classes) est vérifiée à
l'accès depuis E54 — voir
[langage-field-visibility-unchecked](langage-field-visibility-unchecked.md).

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
