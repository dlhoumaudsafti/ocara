# `async` retourne un `int` nu — le vrai fix est un type générique `Resolvable<T>`, pas juste 2 cas manquants

## Constat (vérifié indépendamment)

Trouvé en marge de [langage-async-instance-method-dispatch-broken](langage-async-instance-method-dispatch-broken.md)
en testant les cas généraux du correctif avec un type de retour `string`
plutôt que `int`. Rien à voir avec le codegen d'instance corrigé par ce
ticket — un bug de SEMA (vérification de types), reproduit aussi bien pour
un appel STATIQUE que D'INSTANCE :

```ocara
class Doubler {
    public static async method fetch(): string {
        return "hi"
    }
}
function main(): int {
    var t:int = Doubler::fetch()   // ❌ error: expected type 'int', found 'string'
    var r:string = resolve t        // ❌ error: expected type 'string', found 'int'
    return 0
}
```

Toutes les démonstrations `async` existantes dans ce compilateur (tests,
exemples, la ticket `wiring` elle-même) utilisent `int` comme type de
retour — un choix qui masque ce bug par coïncidence, puisque le type
« réel » et le type « task handle » sont alors identiques.

## Cause immédiate (localisée)

`src/sema/typecheck.rs` n'applique la règle « un appel `async` retourne
`Type::Int` (le task handle), pas son type déclaré » qu'à UN SEUL endroit
(ligne ~1161, dans la résolution `Expr::Call` → fonction LIBRE via
`self.symbols.lookup_function`). Ni `Expr::StaticCall` (`Classe::methode()`)
ni le sucre d'instance (`Expr::Field` en position d'appel,
`objet.methode()`) n'ont l'équivalent — les deux retournent le type de
retour DÉCLARÉ de la méthode telle quelle, jamais `Type::Int`.

## Analyse approfondie — ce n'est pas juste 2 cas manquants dans un `match`

**Constat confirmé en creusant le code** : même en ajoutant la
substitution `is_async → Type::Int` aux deux endroits manquants
(`Expr::StaticCall`, sucre d'instance), le résultat serait *cohérent* mais
toujours *faux* dans l'absolu — un `int` nu ne porte plus aucune information
sur ce que la tâche produit réellement une fois résolue. `t` serait typé
`int` alors qu'il « est » en réalité un `Doubler::fetch(): string` en
attente ; rien n'empêcherait `var x:float = resolve t` de type-checker par
accident si un autre `resolve` quelque part attend un `float`.

Preuve que ce n'est pas théorique : `resolve` **ne fonctionne déjà
aujourd'hui que grâce à un hack fragile**, pas grâce au typage du handle.
Voir `Expr::Resolve` (`typecheck.rs:1924-1947`) : pour retrouver le
« vrai » type de retour, le code consulte `self.async_var_funcs`, une
`HashMap<String, String>` indexée par **nom de variable**, remplie à un
seul site (`typecheck.rs:502-508`) uniquement quand une déclaration
`var`/`scoped`/`consumed` assigne directement le résultat d'un appel à une
fonction LIBRE async. Ce mécanisme casse silencieusement (retombe sur
`Type::Int` par défaut) dès que le handle est :
- réassigné à une autre variable,
- stocké dans un champ, un tableau ou une map,
- passé en paramètre ou retourné par une fonction,
- shadowé dans un scope imbriqué,
- ou produit par un appel STATIQUE/D'INSTANCE (jamais couvert, cf. cause
  immédiate ci-dessus).

Autrement dit : le problème de fond n'est pas « il manque un cas dans un
`match` », c'est que représenter le handle par un `int` nu **efface**
l'information de type au niveau du système de types, et que `resolve`
essaie de la rattraper après coup via une table par nom de variable au lieu
de la porter dans le type lui-même. Étendre la substitution `Type::Int` aux
deux sites manquants rendrait `async` *uniformément* incohérent au lieu
d'*inconsistant* — pas une vraie correction.

## Proposition retenue — un type générique `Resolvable<T>`

au lieu de faire perdre le type réel,
un appel `async` retourne un type générique opaque `Resolvable<T>` où `T` est
le type de retour DÉCLARÉ de la fonction/méthode :

```ocara
class Doubler {
    public static async method fetch(): string {
        return "hi"
    }
}
function main(): int {
    var t:Resolvable<string> = Doubler::fetch()
    var r:string = resolve t
    return 0
}
```

**Pourquoi c'est faisable à faible coût** : l'infrastructure existe déjà.
`Type::Generic { name, args }` (`src/parsing/ast.d/types.rs`) est déjà
utilisé pour les génériques utilisateur (`Cache<K,V>`, etc.). `Resolvable<T>`
serait un générique BUILT-IN de plus, sur le même principe que
`array<T>`/`map<K,V>` : `T` n'existe qu'au niveau du typechecker, jamais
dans la représentation mémoire runtime. Le handle reste exactement ce qu'il
est aujourd'hui (pointeur opaque vers une `OcaraTask`, actuellement
transporté comme `int` par le codegen) — **aucun changement côté
lower/codegen n'est requis pour la représentation runtime elle-même**,
seulement pour les points de résolution de type déjà identifiés
(`Expr::StaticCall`, sucre d'instance) qui, au lieu de forcer `Type::Int`,
doivent produire `Type::Generic { name: "Resolvable", args: vec![ret_ty_declaré] }`.

**Ce que ça élimine** : `resolve` devient une règle générale et
compositionnelle — `resolve` sur une expression de type `Resolvable<T>` a le
type `T`, point final. Plus besoin de `async_var_funcs` : le type `T` est
porté par le type de l'expression elle-même, à n'importe quelle profondeur
d'indirection (variable, champ, tableau, paramètre, valeur de retour) —
exactement la garantie que la table par nom de variable ne peut pas offrir.
La `HashMap` et le cas spécial `Expr::Ident`/`Expr::Call` dans
`Expr::Resolve` (typecheck.rs:1924-1947) peuvent être supprimés entièrement.

**Nommage** : proposition initiale de David `Resolver<T>`, affinée en
`Resolvable<T>` — préféré à `Future<T>`/`Promise<T>`/`Task<T>` (dans la
plupart des langages, ces noms viennent avec une API riche — `.then()`,
combinateurs, annulation — qu'Ocara n'offre pas et ne propose pas d'offrir
ici). `Resolver` (suffixe `-er`) désigne conventionnellement l'agent qui
*effectue* l'action (comme un DNS resolver), pas la valeur en attente
elle-même ; `Resolvable` (« ce qui peut être résolu ») décrit la valeur, pas
un agent, et reste cohérent avec le mot-clé `resolve` déjà existant sans
laisser croire à une API façon callback (`new Promise((resolve, ...))` en
JS, où `resolve` est justement le nom d'un agent).

## Ce qu'il faudra trancher à l'implémentation

- `resolve` sur une expression qui n'est PAS un `Resolvable<T>` : actuellement
  retombe silencieusement sur `Type::Int` (`orig_ty.unwrap_or(Type::Int)`).
  À remplacer par une vraie erreur de type — plus de valeur par défaut
  silencieuse une fois que `Resolvable<T>` porte l'information nécessaire.
- `Resolvable<T>` doit être un type de premier ordre utilisable partout où un
  type l'est normalement (paramètre, propriété, élément de tableau/map) —
  ne PAS le restreindre à la position `var` immédiatement après un appel,
  sous peine de recréer la même faille de composabilité par un autre biais.
- Pas de syntaxe de construction directe d'un `Resolvable<T>` (pas de
  littéral) — uniquement produit par un appel à une fonction/méthode
  `async`, uniquement consommé par `resolve`. Aucune méthode d'instance
  dessus en v1 (pas de `.then()`, pas de combinateurs) — YAGNI, rien ne le
  demande aujourd'hui.
- Migration : usage actuel d'`async`/`resolve` limité à
  `examples/29_async.oc`, `examples/62_interface_method_modifiers.oc`,
  `examples/64_async_instance_method_dispatch.oc` (+ leurs `Test`), et les
  exemples `advanced/httpserver` / `advanced/tauri_httpserver`. Empreinte
  contenue — `mini_project`/`mini_project_hexa` n'utilisent pas `async`.
  Chaque site `var t:int = ...async...` devient `var t:Resolvable<T> = ...`.
- Interaction avec `wiring`/les modificateurs de méthode d'interface :
  orthogonale — la résolution de la cible d'appel (fonction libre, méthode
  statique, méthode d'instance, dispatcher d'interface après `wiring`) se
  fait déjà AVANT la substitution de type de retour ; `Resolvable<T>`
  s'applique uniformément une fois la cible connue, sans cas spécial
  supplémentaire par rapport aux deux sites déjà identifiés.
- Documentation : `docs/EBNF.md` §14.5 décrit aujourd'hui le retour d'un
  `async` comme "une handle de tâche de type `int`" — à réécrire pour
  `Resolvable<T>` si la proposition est retenue et implémentée.
- Casse : les génériques built-in existants sont en minuscule
  (`array<T>`, `map<K,V>`), les génériques utilisateur en PascalCase
  (`Cache<K,V>`). `Resolvable<T>` se comporte comme un type opaque
  « classe », pas un conteneur primitif — garder PascalCase
  (`Resolvable<T>`), à documenter explicitement comme exception assumée à
  la convention minuscule des génériques built-in.
- Imbrication interdite : le type de retour DÉCLARÉ d'une fonction/méthode
  `async` ne doit jamais être lui-même `Resolvable<T>` (empêcher
  `async method fetch(): Resolvable<string>`, qui produirait autrement un
  `Resolvable<Resolvable<string>>` absurde par double emballage implicite)
  — à vérifier en sema à la déclaration.
- Propagation d'erreur (question ouverte liée, hors périmètre de ce
  ticket) : que se passe-t-il si le thread spawné par une fonction `async`
  lève une erreur (`raise`) ? `__task_resolve` fait un `JoinHandle::join`
  aujourd'hui (EBNF §14.5) — à vérifier si un panic dans le thread est géré
  proprement ou fait planter tout le process. Ne bloque pas `Resolvable<T>`
  mais mérite son propre ticket si le comportement actuel s'avère être un
  crash silencieux.

## Priorité / Complexité

**Haute** (bloque un usage légitime documenté — `async` sur une méthode
statique OU d'instance retournant autre chose qu'un `int`, et le mécanisme
actuel de `resolve` est structurellement fragile même pour le cas `int`
qui « marche » aujourd'hui par coïncidence) — **Structurel** : ce n'est
plus une simple extension de substitution à deux sites, mais l'introduction
d'un vrai type générique built-in (`Resolvable<T>`), la suppression du hack
`async_var_funcs`, et la réécriture de la règle de typage de `resolve`.

## Fichiers clés

`src/sema/typecheck.rs` (résolution de type d'`Expr::StaticCall`,
`Expr::Field` en position d'appel, `Expr::Resolve`, suppression
d'`async_var_funcs`), `src/parsing/ast.d/types.rs` (`Type::Generic` déjà
disponible, à instancier avec `name: "Resolvable"`), `docs/EBNF.md` (§14.5,
description du type de retour d'un `async`).
Mettre à jour tous les fichier d'exemple qui utilise resolve

## Résolution (implémentée)

**Écart avec la proposition initiale ci-dessus** : `Resolvable<T>` a été
implémenté comme une variante d'enum DÉDIÉE, `Type::Resolvable(Box<Type>)`
(`src/parsing/ast.d/types.rs`), modelée sur `Type::Message(Box<Type>)` —
PAS comme une instance de `Type::Generic { name: "Resolvable", args }` comme
envisagé plus haut. Raison : `Type::Generic` est réservé aux génériques
UTILISATEUR avec une vraie classe backing (`Cache<K,V>`, monomorphisée en
classe concrète) ; `Resolvable<T>`, comme `array<T>`/`map<K,V>`/`message<T>`,
n'a pas de classe backing — c'est un type built-in au niveau du langage.
Contrairement à `Message<T>` (return-type-only, jamais nommable),
`Resolvable<T>` est un type de PREMIER ORDRE utilisable partout (variable,
paramètre, champ, élément de tableau/map) ; sa seule restriction est de ne
jamais être le type de retour DÉCLARÉ d'une fonction/méthode `async`
elle-même (E44). `resolve` sur autre chose qu'un `Resolvable<T>` est E43.
`Resolvable` n'est pas un mot-clé réservé (reconnu seulement quand suivi de
`<` en position de type).

**Tous les sites de substitution `is_async → Resolvable<T>` couverts** (pas
seulement les 3 initialement identifiés) : fonction libre (`Expr::Call`),
méthode statique (`Expr::StaticCall`), sucre d'instance (`Expr::Field` en
position d'appel), méthode d'un générique instancié, ET référence à une
fonction/méthode async SANS appel (`Expr::Ident`/`Expr::StaticConst`
produisant un `Type::Function{ret_ty: Resolvable<T>, ...}`) — un point non
identifié dans l'analyse initiale.

**Lowering (`src/lower/`)** : contrairement à l'hypothèse initiale (« aucun
changement de représentation runtime requis »), un correctif a été
nécessaire dans `src/lower/expr.d/typeinfer.rs`/`lower.rs` et
`src/lower/stmt.d/statements.d/variables.rs`. Avant Resolvable<T>, la sema
substituait déjà `Type::Int` pour un appel LIBRE async, ce qui masquait un
problème : `expr_ir_type` (utilisé par `box_for_any` pour le boxing `mixed`)
ne savait pas qu'un appel `async` produit toujours un handle `I64`, quel que
soit son type de retour déclaré — il résolvait le type DÉCLARÉ (`string` →
`Ptr`, `float` → `F64`). Avec `Resolvable<T>` permettant `T != int` pour un
appel statique/d'instance, ce mismatch (valeur réelle `I64`, type inféré
`Ptr`/`F64`) aurait fait boxer le handle brut comme un `mixed` via
`__mixed_to_int`/`__box_float`, le corrompant. Corrigé en ajoutant une garde
`is_async_call_target` dans `expr_ir_type` (StaticCall, Call libre, sucre
d'instance) : un appel vers une cible `async` retourne toujours `IrType::I64`
au niveau de l'EXPRESSION D'APPEL elle-même. Le hack symétrique côté
lowering pour `Expr::Resolve` (`async_var_ret`, qui ne suivait que
`var x = <appel direct à une fonction libre>`) a aussi été généralisé pour
dériver le type IR de `T` depuis le type DÉCLARÉ `Resolvable<T>` de la
variable (`register_async_var_ret`), couvrant `const` et toute indirection —
plus seulement `var` assignée directement. Le handle runtime lui-même reste
inchangé (toujours un entier opaque, `IrType::I64`) : ce sont les DÉCISIONS
DE TYPE prises pendant le lowering qui devaient suivre `Resolvable<T>`, pas
la représentation mémoire.

**Vérifié par exécution réelle** (pas seulement compilation) : repro exact
du ticket (`Doubler::fetch(): string` statique ET `d.fetchInstance(): string`
d'instance) + un cas `float` (`Doubler::fetchRatio(): float`) — voir
`examples/65_async_resolvable_return_type.oc` et son test associé.

**Exemples migrés** : `examples/29_async.oc` (+ Test — tous les types de
retour : int/float/bool/string/array/Function/object/map/enum),
`examples/62_interface_method_modifiers.oc` (+ Test),
`examples/64_async_instance_method_dispatch.oc` (+ Test),
`examples/advanced/httpserver/controllers/HomeController.oc`,
`examples/advanced/tauri_httpserver/controllers/HomeController.oc`.
`mini_project`/`mini_project_hexa` n'utilisent pas `async` — non modifiés.

**Documentation** : `docs/EBNF.md` §6.2 (ajout de `ResolvableType` à la
grammaire des types) et §14.5 (réécrit), `docs/diagnostics.md` (E43 —
`resolve` sur non-`Resolvable<T>` ; E44 — retour déclaré `Resolvable<T>`
d'une fonction/méthode `async`), grammaire TextMate VS Code
(`tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`, `Resolvable` coloré
comme type built-in).
