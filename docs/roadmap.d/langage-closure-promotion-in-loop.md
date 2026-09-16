# Une fermeture créée À L'INTÉRIEUR d'une boucle se re-promeut à chaque itération au lieu d'une seule fois

## Constat

Sous-cas résiduel de [langage-closure-promotion-block-scope](langage-closure-promotion-block-scope.md), distinct de celui déjà corrigé (SEGFAULT hors-boucle). Après le correctif de `lower_block`, lire une variable capturée par une fermeture créée dans un bloc `if`/`switch` (passage unique) fonctionne correctement. **Une boucle (`while`/`for`) reste cassée différemment** : pas de crash, mais la fermeture ne voit jamais l'état accumulé des itérations précédentes.

### Reproduction

```ocara
import ocara.IO

class Caller {
    public static method invoke(f:Function<void()>): void { f() }
}

function main(): int {
    var counter:int = 0
    var i:int = 0
    while i smaller 5 {
        Caller::invoke(nameless(): void {
            var inner:Function<void()> = nameless(): void { counter = counter + 1 }
            inner()
        })
        i = i + 1
    }
    IO::writeln(`${counter}`)   // affiche 1, pas 5
    return 0
}
```

## Root cause identifiée précisément (via `ocara --dump`, après le correctif de `lower_block`)

```
bb1 (test de condition, cible du saut arrière) :
    ...
    Branch { cond: ..., then_bb: BlockId(2), else_bb: BlockId(3) }
bb2 (CORPS de la boucle — réexécuté à CHAQUE itération au runtime) :
    Call { dest: Some(Value(7)), func: "__alloc_locked_cell", args: [] }
    Load { dest: Value(8), ptr: Value(0), ty: I64 }          // relit TOUJOURS le slot stack D'ORIGINE
    Call { dest: None, func: "__locked_cell_set", args: [Value(7), Value(8)] }
    ...
    Jump { target: BlockId(1) }                               // retour au test, PUIS rejoue bb2
bb3 (après la boucle) :
    Call { dest: Some(Value(19)), func: "__locked_cell_get", args: [Value(7)] }
```

La séquence de promotion (`__alloc_locked_cell` + copie depuis le slot stack + redirection) est émise UNE SEULE FOIS **à la compilation** (lors du passage sur l'AST de `Expr::Nameless`, forcément unique) — mais comme cette expression est LEXICALEMENT à l'intérieur du corps de boucle, les instructions résultantes atterrissent dans `bb2`, qui est exécuté **à répétition au runtime**. Chaque passage : alloue une NOUVELLE cellule (`__alloc_locked_cell`), la réinitialise en recopiant `Value(0)` — le slot stack ORIGINAL de `counter`, jamais mis à jour après la toute première promotion — puis la fermeture incrémente CETTE cellule fraîche. La cellule de l'itération précédente est abandonnée (fuite mémoire au passage, mais masquée par l'absence de tout mécanisme de libération pour une cellule verrouillée) et son incrément perdu. `bb3`, après la boucle, ne voit que le résultat de la DERNIÈRE itération appliqué à une base toujours remise à zéro.

Cause profonde : le check `if builder.heap_promoted.contains(cap_name) { /* réutiliser */ }` (`src/lower/expr.d/lower.rs:1509`) qui devrait éviter une double promotion est une décision **prise à la compilation** — au moment où `Expr::Nameless` est lowered (une seule fois, quel que soit le nombre d'itérations réelles à l'exécution), `heap_promoted` ne contient pas encore `counter` (c'est justement la première — et unique — fois que ce nœud AST est traversé), donc la branche "promouvoir" est prise. Correct pour du code hors-boucle (exécuté une fois, `bb2`-style ne boucle pas) ; incorrect dès que ce code se retrouve dans un corps de boucle exécuté N fois au runtime avec une seule traversée AST.

## Ce qui est demandé

Pas de correctif tenté ici (diagnostic seulement). Piste à évaluer : dans `lower_while`/`lower_for_in`/`lower_for_map` (`src/lower/stmt.d/statements.d/control_flow.rs`/`loops.rs`), avant de lower le corps, pré-scanner le corps (via une variante de `collect_captures`, déjà rendu transitif par le correctif de [langage-nested-closure-recapture](langage-nested-closure-recapture.md)) pour détecter quelles variables EXTÉRIEURES à la boucle seront capturées par une fermeture quelque part dans le corps — et émettre leur promotion (`__alloc_locked_cell` + copie + `heap_promoted.insert`) UNE SEULE FOIS, AVANT le bloc de la boucle (dans le bloc prédécesseur), plutôt que de laisser la première rencontre AST de `Expr::Nameless` l'émettre à l'intérieur du corps. Une fois pré-promue, le chemin "déjà promu, réutiliser" déjà existant (`lower.rs:1509`) s'applique naturellement et correctement à chaque itération.

Distinguer soigneusement "variable capturée par une fermeture DANS la boucle" (à pré-promouvoir) de "variable simplement utilisée directement dans le corps de la boucle, jamais capturée" (ne doit surtout pas être promue inutilement — coût de performance et changement de représentation sans raison).

Ajouter un test de régression `.oc` dès que corrigé (aucun aujourd'hui — le repro ci-dessus est le point de départ).

## Priorité / Complexité

**Priorité Haute** — silencieux (pas de crash, mais résultat faux), sur un patron d'usage réaliste (compteur/accumulateur incrémenté par une closure à l'intérieur d'une boucle — ex. un callback appelé une fois par élément d'une collection). **Complexité : Dangereuse** — touche le lowering de TOUTES les constructions de boucle, et nécessite un pré-scan supplémentaire avant leur corps ; risque de régression sur la performance (fausse détection de capture) ou de casser un cas déjà fonctionnel si mal isolé.

## Fichiers clés

`src/lower/stmt.d/statements.d/control_flow.rs` (`lower_while`), `src/lower/stmt.d/statements.d/loops.rs` (`lower_for_in`, `lower_for_map`), `src/lower/expr.d/lower.rs` (`Expr::Nameless`, la logique de promotion à hoister), `src/lower/expr.d/captures.rs` (`collect_captures`, transitif, réutilisable pour le pré-scan), [langage-closure-promotion-block-scope](langage-closure-promotion-block-scope.md) (ticket parent, cas hors-boucle corrigé), [langage-nested-closure-recapture](langage-nested-closure-recapture.md) (mécanisme de capture transitif déjà en place).
