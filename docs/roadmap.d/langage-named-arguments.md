# Arguments nommés à l'appel (fonction, méthode, `use Classe(...)`, `struct`...)

## Demande (David)

```ocara
var user:UserDTO = use UserDto(
    id: 42,
    name: 'David',
    email: 'david@example.com',
);
```

Doit continuer à fonctionner en purement positionnel :

```ocara
var user:UserDTO = use UserDto(42, 'David', 'david@example.com');
```

Et en nommé, dans un ordre différent de la déclaration — résolu par nom de
paramètre, pas par position :

```ocara
var user:UserDTO = use UserDto(
    id: 42,
    email: 'david@example.com',
    name: 'David',
);
```

Existe en PHP (arguments nommés natifs, PHP 8.0+). **Nuance à noter** :
TypeScript n'a PAS d'arguments nommés natifs — l'ergonomie équivalente y
vient de la déstructuration d'un unique paramètre objet
(`function f({name, id}: {...})`), un mécanisme différent (un seul
paramètre objet, pas un appel à plusieurs arguments nommés individuellement).
Le modèle demandé ici correspond à PHP, pas à TypeScript.

## État actuel (vérifié dans le code)

Aucune forme nommée n'existe à l'appel — `ArgList ::= Expression ( ","
Expression )*` (`docs/EBNF.md` §14/§16/§25, répété identiquement pour
`FuncDecl`, méthode, `NewExpr`/`use`, `StaticCall`) : purement positionnel,
partout, sans exception.

En revanche, la valeur par défaut sur un paramètre **existe déjà** et
s'articule naturellement avec la demande : `Param ::= Identifier ":"
(Type ("=" Expr)?)`, contrainte « les paramètres avec valeur par défaut
doivent être placés après les obligatoires » (`docs/EBNF.md` §14). Les
arguments nommés sont ce qui donne enfin de la valeur à cette contrainte
d'ordre : sans eux, un paramètre optionnel non-dernier ne peut être atteint
qu'en fournissant TOUS les paramètres qui le précèdent, y compris ceux avec
défaut qu'on voudrait justement laisser à leur valeur par défaut.

## Points à trancher avant d'implémenter

- **Mélange positionnel + nommé dans le même appel** — autorisé (règle PHP
  classique : les positionnels doivent tous précéder les nommés) ou
  interdit (un appel est soit 100 % positionnel, soit 100 % nommé, plus
  simple à vérifier et moins ambigu à lire) ?
- **Portée du mécanisme** — s'applique à un appel direct dont les noms de
  paramètres sont statiquement connus depuis la déclaration (fonction libre,
  méthode statique/d'instance, `init()` via `use Classe(...)`). Ne peut PAS
  s'appliquer à un appel à travers une valeur de type `Function<T(...)>` :
  ce type ne référence que les TYPES des paramètres, jamais leurs noms
  (`Function<ReturnType(ParamType1, ParamType2, ...)>`, `docs/EBNF.md`) — à
  documenter explicitement comme limite assumée, pas un oubli.
- **Paramètre variadic** — ciblable par nom, ou seulement positionnel (comme
  c'est déjà le cas pour la valeur par défaut, qu'un variadic ne peut pas
  avoir) ? Recommandation : rester cohérent, variadic = positionnel
  uniquement.
- **Argument fourni deux fois** (une fois par position, une fois par nom
  pour le même paramètre) — erreur de compilation explicite à spécifier.
- **Conséquence assumée** : renommer un paramètre devient un changement
  cassant pour tout site d'appel utilisant son nom — le nom du paramètre
  entre de fait dans le contrat public de la fonction/méthode/constructeur,
  plus seulement sa position et son type. À documenter clairement (comme en
  PHP), pas un défaut caché.
- **Diagnostic sur nom inconnu** — `UserDto(prenom: 'David', ...)` où le
  paramètre s'appelle en réalité `name` : erreur de compilation dédiée, avec
  suggestion du nom le plus proche si un mécanisme de ce type existe déjà
  ailleurs dans les diagnostics (sinon simple liste des noms valides).

## Prise en charge dans l'extension VS Code (`tools/highlight/vsode/`)

Fait partie du ticket (pas une suite optionnelle) — sans ça, la
fonctionnalité est utilisable mais pénible à écrire (il faut connaître les
noms de paramètres par cœur). État actuel vérifié : l'extension n'enregistre
qu'un `DefinitionProvider` et un `CompletionItemProvider` (déclencheurs `.`
et `:`, `src/extension.ts`), aucun `SignatureHelpProvider` ni
`InlayHintsProvider`. Les noms de paramètres sont déjà extraits (builtins via
`BuiltinParam` dans `data/`, méthodes utilisateur via `m.params` dans
`src/resolver.ts`), mais uniquement affichés dans le `detail` d'un item de
complétion — jamais proposés à l'intérieur des parenthèses d'un appel.

À ajouter :

- **Coloration** (`syntaxes/ocara.tmLanguage.json`) — reconnaître
  `Identifier ":"` à l'intérieur d'une liste d'arguments d'appel comme nom de
  paramètre (scope du type `variable.parameter.named-argument.ocara`), sans
  le confondre avec `::` (appel statique) ni avec l'annotation de type
  `nom:Type` d'une déclaration (`var`, `Param`).
- **Complétion des noms de paramètres** — à l'intérieur de `f(`, `obj.m(`,
  `Classe::m(`, `use Classe(` : proposer `nom: ` pour chaque paramètre de la
  cible résolue (fonction libre, méthode, `init()` pour `use`), en excluant
  ceux déjà fournis dans l'appel (par nom ou par position), variadic exclu si
  la règle « variadic = positionnel uniquement » est retenue. Attention au
  déclencheur `:` déjà enregistré (pensé pour `::`) : ne pas proposer de
  complétion de membre statique après un `:` simple de nom d'argument.
- **Signature help** (nouveau `SignatureHelpProvider`, déclencheurs `(` et
  `,`) — afficher la signature complète avec le paramètre actif surligné,
  déterminé par **nom** quand l'argument courant est nommé, par position
  sinon ; indiquer les valeurs par défaut (`= expr`) pour montrer ce qui peut
  être omis.
- **Go-to-definition sur le nom d'argument** — `name:` dans
  `use UserDto(name: 'David')` → saut vers le paramètre `name` de `init()`
  (le `DefinitionProvider` existant résout déjà la cible de l'appel).
- **Appel via `Function<T(...)>`** — aucune complétion de nom (limite du
  langage, cf. plus haut) : ne rien proposer plutôt que des noms inventés.
- Procédure habituelle (`docs/roadmap.md` § Méthode de travail) : lire le
  README de l'extension, désinstaller, recompiler sans changer la version,
  réinstaller.

## Remarque annexe (hors périmètre de CE ticket)

La syntaxe de champ `id:int|null`, `name:string = 'John'` (sans
`public`/`property`) est désormais la syntaxe RETENUE pour
[langage-struct-value-type](langage-struct-value-type.md) (champ nu =
public implicite, `protected` explicite si besoin, `private` rejeté), avec
un mécanisme de valeur par défaut partagé avec
[langage-property-initializer](langage-property-initializer.md). Ce
ticket-ci ne concerne QUE la syntaxe d'appel (nommage des arguments) —
c'est ce qui permet de ne fournir que les champs obligatoires d'un struct
(ceux sans valeur par défaut) sans respecter l'ordre de déclaration pour les
autres.

## Priorité / Complexité

**Haute** (demandé explicitement) — **Structurel** : extension de la
grammaire d'`ArgList` (accepter `Identifier ":" Expression` en plus
d'`Expression` nue) et de la résolution d'appel en sema pour tous les sites
(`FuncDecl`, méthode, `init`/`use`, `StaticCall`) — mécanique une fois la
grammaire décidée, mais à répliquer sur tous les points d'appel existants,
et à faire cohabiter proprement avec les valeurs par défaut déjà en place.

## Fichiers clés

`docs/EBNF.md` (`ArgList`, plusieurs occurrences identiques),
`src/parsing/parser.d/expressions.rs` (parsing des appels),
`src/parsing/ast.d/expressions.rs` (représentation d'un argument, nommé ou
non), `src/sema/typecheck.rs` (résolution appel → paramètres par nom au lieu
de la position pure), `tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`,
`tools/highlight/vsode/src/completion.ts`, `tools/highlight/vsode/src/extension.ts`,
`tools/highlight/vsode/src/resolver.ts` (outillage VS Code).
