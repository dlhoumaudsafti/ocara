# Une fermeture imbriquée dans une fermeture ne recapture pas ce que son parent capture

## ✅ Corrigé

Voir §"Correctif appliqué" plus bas. `examples/builtins/httpserver.oc` utilise maintenant le patron `withLock` imbriqué recommandé (plus de contournement), et un test de régression dédié existe (`examples/tests/46_nested_closure_recaptureTest.oc`).

## Constat

Découvert en écrivant l'exemple `/hits` de `docs/roadmap.d/runtime-httpserver-race-condition.md` : `hitLock.withLock(nameless(): void { hitCount = hitCount + 1 })` **à l'intérieur** d'un handler `nameless(req:int): int { ... }` ne modifiait jamais `hitCount` — reste silencieusement à `0`, aucune erreur de compilation, aucun crash.

Isolé en dehors de tout `HTTPServer`/`Mutex` avec un repro minimal (voir §"Reproduction") : **toute fermeture (`nameless`) définie à l'intérieur d'une autre fermeture perd la capacité de muter une variable que la fermeture englobante n'a fait QUE capturer (pas déclarer localement) depuis un niveau encore supérieur.** Une fermeture à un seul niveau de profondeur fonctionne toujours correctement (capture directe, mutation visible immédiatement des deux côtés — comportement documenté en `docs/EBNF.md` §14.4/§12.2). C'est spécifiquement la **profondeur 2+** qui casse.

### Reproduction (isolée, sans Mutex/HTTPServer)

```ocara
import ocara.IO

class Caller {
    public static method invoke(f:Function<void()>): void { f() }
}

function main(): int {
    var counter:int = 0
    Caller::invoke(nameless(): void {
        var inner:Function<void()> = nameless(): void {
            counter = counter + 1
        }
        inner()
    })
    IO::writeln(`${counter}`)   // affiche 0, pas 1
    return 0
}
```

Variante « niveau 2 capture un LOCAL du niveau 1 » (pas une capture-du-niveau-1-elle-même) : **fonctionne correctement** — ce n'est donc pas la profondeur d'imbrication en tant que telle qui pose problème, c'est spécifiquement la chaîne « niveau 2 a besoin d'une variable que niveau 1 n'a lui-même que capturée, pas déclarée ».

## Root cause identifiée précisément (via `ocara --dump` sur le repro)

`collect_captures` (`src/lower/expr.d/captures.rs:103-104`) refuse explicitement de descendre dans une `Expr::Nameless` imbriquée lors du calcul des captures de la fermeture englobante :
```rust
// Ne pas descendre dans les nameless imbriquées (elles ont leurs propres captures)
Expr::Nameless { .. } | Expr::Literal(..) | Expr::StaticConst { .. } => {}
```
C'est correct EN ISOLATION (la fermeture englobante n'a pas besoin, pour SON PROPRE corps, d'une variable utilisée seulement par sa fermeture enfant) — mais ça signifie que la fermeture englobante ne capture/ne transmet JAMAIS une variable dont seule sa fermeture enfant a besoin. Quand vient le tour de lower la fermeture ENFANT (`src/lower/expr.d/lower.rs:1469-1479`, bloc `Expr::Nameless` qui fusionne `builder.locals` + `builder.captured_vars` avant son propre appel à `collect_captures` — fusion déjà ajoutée par un correctif antérieur pour un bug de la MÊME famille, voir le commentaire à la ligne 1470-1473), la variable manquante n'est ni dans `locals` ni dans `captured_vars` DE LA FERMETURE ENGLOBANTE, puisque celle-ci ne l'a jamais elle-même capturée — invisible à ce stade, quelle que soit la fusion appliquée.

Confirmé par `ocara --dump` sur le repro minimal : la fermeture englobante (`__anon_0`) capture correctement `m` (`__cap_0`, seule variable qu'elle référence directement) mais PAS `counter`. La fermeture enfant (`__anon_1`, le corps du `withLock`) est alors créée avec :
```
ConstInt { dest: Value(6), value: 0 }
...
SetField { obj: Value(8), field: "env", src: Value(6), offset: 8 }
```
— son pointeur d'environnement est un `ConstInt 0` (NULL) littéral : `collect_captures(body_de___anon_1, ...)` a retourné une liste de captures VIDE (le chemin `captures.is_empty() && !has_defaults` de `src/lower/expr.d/lower.rs:1577-1580` est pris), alors que `counter` est bien référencée dans son corps. La fermeture enfant tourne donc avec un environnement nul — toute lecture/écriture de `counter` à l'intérieur retombe sur un comportement dégradé silencieux (valeur figée, jamais la cellule heap partagée réelle) plutôt que sur un crash, ce qui explique l'absence de tout signal d'erreur observable.

## Portée du bug

Touche potentiellement toute fermeture à profondeur 2+ qui référence une variable capturée (pas locale) par son parent immédiat — un patron naturel et pas rare : `m.withLock(nameless(){...})` (le patron recommandé pour toute section critique, voir `docs/builtins/Mutex.md`) à l'intérieur d'un handler `HTTPServer`/`Thread`, un `match`/`if` avec fermeture imbriquée dans un callback, etc. Aucun des exemples existants du projet (`examples/`) ne semble exercer ce cas précis (`m.withLock` n'apparaît qu'au premier niveau dans `examples/builtins/mutex.oc`) — c'est probablement pour ça qu'il n'avait jamais été détecté jusqu'ici.

## Correctif appliqué

`collect_captures` (`src/lower/expr.d/captures.rs`) rendu transitif : le cas `Expr::Nameless` (au lieu d'ignorer purement et simplement une fermeture imbriquée) calcule maintenant RÉCURSIVEMENT ses propres captures (`collect_captures(body, nested_params, l)`, même `l` que le niveau courant) et remonte dans les captures du niveau courant tout nom qu'elle référence, visible dans `l`, et pas déjà l'un de nos propres paramètres. Récursif par construction : une fermeture imbriquée à N niveaux fait remonter son besoin à travers chaque niveau intermédiaire, un par un, jusqu'à la fermeture qui la capture réellement pour la première fois. Aucune autre modification nécessaire — le mécanisme de threading des valeurs (`capture_vals` dans `src/lower/expr.d/lower.rs`) gérait déjà correctement les deux cas ("variable locale à promouvoir" et "variable déjà capturée par le parent, `GetField` depuis son env"), seule la LISTE des captures à calculer était incomplète.

Vérifié : les 3 repros du §"Reproduction" (imbrication directe, `Caller::invoke`+`withLock`, 3 niveaux) donnent maintenant le résultat attendu. Non-régression confirmée sur le cas qui fonctionnait déjà (niveau 2 capturant un LOCAL de niveau 1, pas une capture de niveau 1). `cargo test -p ocara --bin ocara` : 56 passed, 0 warning. `make regression` : 642 PASS, 0 FAIL (dont le nouveau test dédié ci-dessous).

Nouveau test de régression : `examples/tests/46_nested_closure_recaptureTest.oc` (5 assertions — imbrication directe, `withLock` imbriqué, 3 niveaux, non-régression capture-de-local, appels répétés). Volontairement AUCUN de ces tests ne place la création de fermeture à l'intérieur d'un bloc `if`/`while`/`for` — voir la découverte connexe ci-dessous.

`examples/builtins/httpserver.oc` mis à jour pour utiliser `hitLock.withLock(nameless(){...})` (le patron recommandé) au lieu du contournement `lock()`/`unlock()` manuels appliqué avant ce correctif — revérifié par le même test de charge concurrente (30 requêtes simultanées, aucune incrémentation perdue), voir `docs/roadmap.d/runtime-httpserver-race-condition.md`.

## ⚠️ Découverte en vérifiant ce correctif — bug distinct, PLUS sévère, non corrigé

En testant des variantes du repro, un cas séparé a été trouvé : une fermeture à **un seul niveau** (aucune imbrication), créée à l'intérieur d'un bloc `if`/`while`/`for`, **SEGFAULT** dès que la variable qu'elle capture est relue après le bloc — root cause différente (la redirection heap-promotion de `builder.locals` ne survit pas à la sortie du bloc), sans rapport avec la recapture transitive corrigée ici. Documenté séparément : [langage-closure-promotion-block-scope](langage-closure-promotion-block-scope.md).

## Priorité / Complexité

**✅ Terminé.** Était Priorité Haute (bug de correction silencieux, patron d'usage naturel déjà recommandé dans la doc du projet). **Complexité réelle : Légère** — contrairement à l'estimation initiale ("Dangereuse"), le correctif s'est avéré localisé à une seule fonction (`collect_captures`) sans toucher au mécanisme de threading des valeurs, qui était déjà correct.

## Fichiers clés

`src/lower/expr.d/captures.rs` (`collect_captures`, `walk_expr_caps` — corrigé), `src/lower/expr.d/lower.rs` (`Expr::Nameless`, lignes ~1464-1600, inchangé), `examples/tests/46_nested_closure_recaptureTest.oc` (fait), `examples/builtins/httpserver.oc` (mis à jour, contournement retiré), `docs/EBNF.md` §14.4/§12.2 (sémantique documentée des closures), [langage-closure-promotion-block-scope](langage-closure-promotion-block-scope.md) (bug connexe découvert, non corrigé).
