# SEGFAULT : une fermeture créée dans un bloc `if`/`while`/`for` perd la promotion heap de la variable qu'elle capture, dès qu'on la relit après le bloc

## ✅ Corrigé pour le cas `if`/`switch` (passage unique) — voir §"Correctif appliqué"

Le cas boucle (`while`/`for`) a une root cause DIFFÉRENTE, non corrigée ici : voir [langage-closure-promotion-in-loop](langage-closure-promotion-in-loop.md).

## Constat

Découvert en testant le correctif de [langage-nested-closure-recapture](langage-nested-closure-recapture.md) : contrairement à ce que ce dernier ticket pouvait laisser penser, le problème ne se limite pas à l'imbrication de fermetures — **une fermeture à un seul niveau, si elle est simplement créée à l'intérieur d'un bloc `if`/`while`/`for`, plante en SEGFAULT dès que la variable qu'elle capture est relue APRÈS le bloc.**

### Reproduction minimale

```ocara
import ocara.IO

function main(): int {
    var counter:int = 0
    if true {
        var f:Function<void()> = nameless(): void { counter = counter + 1 }
        f()
    }
    IO::writeln(`${counter}`)   // SEGFAULT ici
    return 0
}
```

`./ocara repro.oc -o repro && ./repro` → `Erreur de segmentation (core dumped)`.

Variante boucle (`while`, pas de crash mais résultat silencieusement faux — 0 au lieu de 5) :
```ocara
var counter:int = 0
var i:int = 0
while i smaller 5 {
    var f:Function<void()> = nameless(): void { counter = counter + 1 }
    f()
    i = i + 1
}
IO::writeln(`${counter}`)   // affiche 0, pas 5 — aucun crash, juste faux
```

Le fait qu'un cas plante et l'autre se contente d'un résultat faux suggère un accès mémoire invalide dont la conséquence dépend du contenu du tas à cet endroit précis (comportement non défini, pas une différence de nature entre les deux cas).

## Root cause identifiée précisément (via `ocara --dump`)

Sur le repro `if`, l'IR généré pour `main` :
```
bb0: Alloca { dest: Value(0), ty: I64 }        // counter, stack, AVANT le if
     ...
bb1 (corps du if) :
     Call { dest: Some(Value(4)), func: "__alloc_locked_cell", args: [] }
     ...
     builder.locals["counter"] redirigée vers Value(4) (voir
     src/lower/expr.d/lower.rs:1538, `builder.locals.insert(cap_name, (heap_ptr, ...))`)
bb3 (après le if, `IO::writeln(counter)`) :
     Call { dest: Some(Value(21)), func: "__locked_cell_get", args: [Value(0)] }
                                                  ^^^^^^^ toujours Value(0), le
                                                  slot stack D'ORIGINE, jamais
                                                  redirigé vers Value(4)
```

`bb3` appelle `__locked_cell_get` (qui attend un pointeur de cellule verrouillée, alloué par `__alloc_locked_cell`) avec `Value(0)` — le pointeur de la `Alloca` stack ORIGINALE de `counter`, jamais promue à cet endroit. Confusion de représentation : `__locked_cell_get` déréférence `Value(0)` comme si c'était une cellule verrouillée valide, alors que c'est un slot stack ordinaire — accès mémoire invalide, SEGFAULT.

Cause : la redirection `builder.locals.insert("counter", (heap_ptr, ...))` faite lors de la création de la fermeture (§`Expr::Nameless`, `src/lower/expr.d/lower.rs:1538`) a lieu PENDANT le lowering du bloc `then` du `if` — mais cette mise à jour de `builder.locals` **ne survit pas à la sortie du bloc** : au moment de lower le code APRÈS le `if` (`bb3`), `builder.locals["counter"]` a été restaurée (ou n'a jamais été propagée) à son entrée d'AVANT le bloc (`Value(0)`, jamais promue). Suggère que le lowering d'un bloc (`if`/`while`/`for` — voir `src/lower/stmt.d/block.rs`, `lower_if`/`lower_while`/`lower_for_in`) sauvegarde/restaure `builder.locals` à ses frontières (probablement pour que les déclarations LOCALES AU bloc ne fuient pas vers l'extérieur, ce qui est correct) — mais applique cette même restauration, À TORT, aux entrées qui existaient DÉJÀ avant le bloc et qui ont seulement été REDIRIGÉES (promotion heap), pas nouvellement déclarées.

**Confirmé après correctif** : le même mécanisme touchait bien `if`/`switch`/`try` (tous passent par `lower_block`, voir plus bas) — corrigé une seule fois, pour tous, en même temps. La variante boucle avait été correctement anticipée ici comme « plausible » mais s'est avérée être une root cause DIFFÉRENTE (pas la même restauration fautive) — voir [langage-closure-promotion-in-loop](langage-closure-promotion-in-loop.md).

## Portée

Plus sévère que [langage-nested-closure-recapture](langage-nested-closure-recapture.md) : ne nécessite AUCUNE imbrication de fermeture, juste une fermeture ordinaire à un seul niveau, créée dans n'importe quel bloc conditionnel/boucle, dont la variable capturée est relue après. C'est un patron extrêmement courant (`if condition { ... var f = nameless(){...}; f() ... }`, ou plus simplement toute closure créée conditionnellement). Risque réel de SEGFAULT en production sur du code par ailleurs parfaitement raisonnable.

## Correctif appliqué

`lower_block` (`src/lower/stmt.d/block.rs`) prenait un instantané de `builder.locals` à l'entrée du bloc et le restaurait AVEUGLÉMENT en sortie — objectif légitime (empêcher une variable déclarée DANS le bloc de fuiter vers l'extérieur, `builder.locals` étant une table plate, pas une pile de scopes) mais appliqué à TORT aux entrées qui existaient déjà avant le bloc et qui ont seulement été redirigées vers une cellule heap (promotion par capture de fermeture), pas nouvellement déclarées.

Corrigé en excluant de la restauration toute entrée dont le nom est dans `builder.heap_promoted` (mise à jour par `Expr::Nameless` lors d'une promotion, `src/lower/expr.d/lower.rs:1539` — délibérément non scopée par bloc : une fois promu, un nom reste heap-backed pour le reste de la fonction). C'est exactement le signal qui distingue une redirection légitime (à garder) d'un nouveau `var`/`scoped`/`consumed` de même nom qui masque ce nom (cas normal de masquage, toujours restauré correctement).

`lower_if`/`lower_switch`/`lower_while`/`lower_for_in`/`lower_for_map`/`lower_try` appellent tous `lower_block` pour leur(s) bloc(s) — un seul correctif dans `lower_block` couvre uniformément toutes ces constructions, sans dupliquer la logique.

Vérifié : les 2 repros `if` (variante `Function<void()>` directe et variante avec fermeture imbriquée) donnent maintenant le résultat attendu, sans crash. `cargo test -p ocara --bin ocara` : 56 passed. `make regression` : 648 PASS, 0 FAIL (dont le nouveau test dédié ci-dessous).

Nouveau test de régression : `examples/tests/47_closure_promotion_block_scopeTest.oc` (6 assertions — fermeture dans `if`, dans la branche `else`, fermeture imbriquée dans un `if`, non-régression du masquage SANS promotion, fermeture déjà promue AVANT le bloc). Volontairement AUCUN cas boucle — voir la découverte connexe ci-dessus.

## Priorité / Complexité

**✅ Terminé pour le cas hors-boucle.** Était Priorité Haute (SEGFAULT sur un patron de code très courant). **Complexité réelle : Légère** — contrairement à l'estimation initiale ("Dangereuse"), le correctif tient en une condition supplémentaire dans une seule fonction déjà bien isolée (`lower_block`), sans toucher au reste du lowering des blocs. Le cas boucle reste ouvert séparément, voir [langage-closure-promotion-in-loop](langage-closure-promotion-in-loop.md) (celui-là, véritablement plus invasif).

## Fichiers clés

`src/lower/stmt.d/block.rs` (`lower_block` — corrigé), `src/lower/stmt.d/statements.rs`/`statements.d/control_flow.rs`/`statements.d/loops.rs` (`lower_if`, `lower_switch`, `lower_while`, `lower_for_in`, `lower_for_map` — tous appellent `lower_block`, inchangés), `src/lower/expr.d/lower.rs` (`Expr::Nameless`, `heap_promoted`, inchangé), `examples/tests/47_closure_promotion_block_scopeTest.oc` (fait), [langage-nested-closure-recapture](langage-nested-closure-recapture.md) (bug connexe, mécanisme de capture), [langage-closure-promotion-in-loop](langage-closure-promotion-in-loop.md) (cas boucle, non corrigé).
