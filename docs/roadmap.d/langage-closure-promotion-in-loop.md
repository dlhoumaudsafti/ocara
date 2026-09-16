# Une fermeture créée À L'INTÉRIEUR d'une boucle se re-promeut à chaque itération au lieu d'une seule fois

## ✅ Corrigé

Voir §"Correctif appliqué" plus bas.

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

## Correctif appliqué

Exactement la piste envisagée ci-dessus, implémentée telle quelle :

1. **Pré-scan** : nouvelle fonction publique `names_captured_by_nested_closures` (`src/lower/expr.d/captures.rs`), qui partage les mêmes fonctions de parcours internes que `collect_captures` (un paramètre `count_direct_refs: bool` ajouté et discrètement threadé partout — `collect_captures` garde exactement son comportement/signature d'avant, `true` ; la nouvelle fonction utilise `false`). `false` fait ignorer les références DIRECTES du bloc scanné (le compteur `i` d'un `while i smaller N {...}` n'est jamais confondu avec une capture) — seules les captures de fermetures RÉELLEMENT créées dans le bloc, à n'importe quelle profondeur (`Expr::Nameless`, déjà rendu transitif par le correctif de [langage-nested-closure-recapture](langage-nested-closure-recapture.md)), sont remontées.
2. **Hoist** : nouvelle fonction `hoist_closure_promotions_before_loop` (`src/lower/expr.d/lower.rs`) — appelle le pré-scan, puis pour chaque nom pas déjà `heap_promoted`, émet la MÊME séquence de promotion que `Expr::Nameless` (`__alloc_locked_cell` + copie + redirection + `heap_promoted.insert`), volontairement DUPLIQUÉE plutôt que refactorisée avec le code existant (déjà testé) — un ajout pur à un nouvel endroit, risque minimal sur le chemin déjà validé.
3. **Câblage** : appelée en tout premier, AVANT toute déclaration locale propre à la boucle (variable d'itération, index...), dans `lower_while`, `lower_for_in` (après le court-circuit générateur, qui a son propre mécanisme) et `lower_for_map` (`src/lower/stmt.d/statements.d/control_flow.rs`/`loops.rs`).

Une fois pré-promue AVANT le bloc de la boucle (donc une seule fois, dans le bloc prédécesseur — confirmé par `ocara --dump`), le chemin "déjà promu, réutiliser" déjà existant dans `Expr::Nameless` s'applique naturellement à chaque itération : aucune ré-allocation, la même cellule est relue/écrite à chaque passage.

Vérifié : le repro `while` donne maintenant `5` (plus `1`). Étendu et vérifié aussi pour `for x in array` (accumulateur ET variable d'itération capturés), `for k => v in map`, boucles imbriquées (fermeture dans la boucle interne), et non-régression explicite pour une boucle SANS AUCUNE fermeture (pas de promotion superflue déclenchée par le pré-scan). `cargo test -p ocara --bin ocara` : 56 passed. `make regression` : 653 PASS, 0 FAIL (dont le nouveau test dédié ci-dessous).

Nouveau test de régression : `examples/tests/48_closure_promotion_in_loopTest.oc` (5 assertions — `while`, `for-in`, `for-map`, non-régression sans fermeture, boucles imbriquées).

## Priorité / Complexité

**✅ Terminé.** Était Priorité Haute (silencieux — pas de crash, mais résultat faux — sur un patron d'usage réaliste). **Complexité réelle : Dangereuse confirmée** (seule fiche de ce lot où l'estimation initiale s'est avérée juste) — touche le lowering de `while`/`for-in`/`for-map`, nécessite un pré-scan supplémentaire distinct du mécanisme de capture existant ; budget de tests de non-régression large effectivement nécessaire (vérifié sur les 3 formes de boucle + imbrication + cas sans fermeture, pas seulement le repro `while` initial).

## Fichiers clés

`src/lower/expr.d/captures.rs` (`names_captured_by_nested_closures`, fait), `src/lower/expr.d/lower.rs` (`hoist_closure_promotions_before_loop`, fait), `src/lower/stmt.d/statements.d/control_flow.rs` (`lower_while`, câblé), `src/lower/stmt.d/statements.d/loops.rs` (`lower_for_in`, `lower_for_map`, câblés), `examples/tests/48_closure_promotion_in_loopTest.oc` (fait), [langage-closure-promotion-block-scope](langage-closure-promotion-block-scope.md) (ticket parent, cas hors-boucle), [langage-nested-closure-recapture](langage-nested-closure-recapture.md) (mécanisme de capture transitif réutilisé).
