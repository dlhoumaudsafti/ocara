# `Array::get(arr, i)` mal affiché quand utilisé directement comme argument

## Découverte

Trouvé en vérifiant `Array::fromMessage` (voir docs/roadmap.d/langage-emit-iterable.md) — bug **pré-existant, sans rapport avec `emit`/les générateurs** : reproductible avec un simple littéral `array<int>`, aucun générateur impliqué.

## Reproduction

```ocara
import ocara.IO
import ocara.Array

function main(): int {
    var values:array<int> = [0, 1, 2, 3]
    var i:int = 0
    while i smaller Array::len(values) {
        IO::writeln(Array::get(values, i))   // ❌ affiche "null" pour l'index 0
        i = i + 1
    }
    return 0
}
```

Affiche `null / 1 / 2 / 3` au lieu de `0 / 1 / 2 / 3`.

**Contournement déjà disponible** (fonctionne correctement) :

```ocara
var x:int = Array::get(values, i)
IO::writeln(x)   // ✅ affiche bien 0, 1, 2, 3
```

## Cause

`program.rs` enregistre `fn_ret_types.insert("Array_get", IrType::Ptr)` — correct pour `array<mixed>` (éléments déjà boxés), mais FAUX pour un `array<T>` à élément CONCRET (`int`/`float`/`bool`) : la valeur brute `0` est alors interprétée comme un pointeur nul par le dispatch typé de `IO::write`/`IO::writeln` (`write_variant`, basé sur `expr_ir_type`), qui affiche `null`.

`expr_ir_type` (`src/lower/expr.d/typeinfer.rs`) sait déjà résoudre le type d'élément CONCRET d'une variable tableau via `builder.elem_types` (voir son traitement de `Expr::Index`) — mais son traitement de `Expr::StaticCall` pour `Array::get`/`Array::first`/`Array::last`/`Array::pop` (retour actuellement toujours `Ptr` via `fn_ret_types`) ne consulte jamais cette information, contrairement à un accès par indexation directe (`arr[i]`).

## Piste de correction

Dans `expr_ir_type` (`Expr::StaticCall`), pour `Array::get`/`first`/`last`/`pop` (et probablement les équivalents `Map::*`), résoudre le type CONCRET de l'élément via `builder.elem_types`/`builder.elem_ast_types` de l'objet passé en premier argument (même logique que `Expr::Index`), avant de retomber sur le `Ptr` générique de `fn_ret_types` si l'objet n'est pas une variable à type d'élément concret connu.

## Priorité

Moyenne — bug de fiabilité réel (affichage silencieusement faux), mais avec un contournement simple déjà disponible (variable intermédiaire typée) et un périmètre de correction visiblement circonscrit à `expr_ir_type`.
