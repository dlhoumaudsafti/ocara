# Arithmétique incorrecte entre `int` et une valeur `mixed`

## Constat

Découvert en écrivant la documentation de `Map::forEach` (voir [builtins-map-foreach](builtins-map-foreach.md)). Une addition (`+`) entre un `int` et une variable `mixed` qui contient réellement un entier produit un résultat numériquement faux — silencieusement, sans erreur ni avertissement au-delà du warning habituel "type 'mixed' disables type checking".

## Reproduction

```ocara
var x:mixed = 95
var y:int = 10
var z:int = y + x
IO::writeln(z)   // affiche 1062449896 au lieu de 105
```

Le narrowing `if x is int { ... }` ne corrige pas le problème à l'intérieur du bloc :

```ocara
if x is int {
    var z:int = y + x   // toujours faux à l'intérieur du bloc narrowed
}
```

**Cause probable** : `Type::Mixed` est réduit à `IrType::Ptr` au niveau de l'IR (`src/ir/types.rs::from_ast`), y compris pour une valeur qui est en réalité un petit entier stocké tel quel (non boxé, sous `PTR_THRESHOLD`). L'addition émise traite vraisemblablement `x` comme un pointeur/registre de type différent d'`I64` plutôt que la valeur entière brute qu'il contient réellement — contrairement aux comparaisons strictes (`equal`, `smaller`, ...) qui, elles, passent par un runtime dispatch dédié (`__cmp_eq_strict` etc., voir `src/lower/expr.d/lower.rs`) quand un opérande est `mixed`. Il n'existe pas d'équivalent runtime dispatch pour `+`/`-`/`*`/`/` avec un opérande `mixed`.

**Contournement fonctionnel vérifié** : passer par une conversion explicite via string plutôt qu'une addition directe :

```ocara
var xi:int = Convert::strToInt(`${x}`)
var z:int = y + xi   // 105, correct
```

## Portée

Touche potentiellement tout code qui fait de l'arithmétique sur une valeur `mixed` contenant un entier — par exemple accumuler les valeurs d'un `map<string, mixed>` via `Map::forEach`/`Map::values`, ou tout paramètre/retour `mixed` utilisé ensuite dans un calcul.

## Ampleur

Non évalué en détail (découverte en cours de route, pas d'investigation du chemin de lowering de `Inst::Add` avec un opérande `Ptr`/`mixed`) — probablement du même ordre que l'ajout des comparaisons strictes (un dispatch runtime dédié pour les opérations arithmétiques avec un opérande `mixed`), donc Structurel plutôt que Légère.

## Fichiers clés

`src/ir/types.rs` (`IrType::from_ast`), `src/lower/expr.d/lower.rs` (comparaisons strictes existantes à prendre comme modèle), `src/codegen/emit.d/instructions.d/arithmetic.rs`.
