# Variables extérieures modifiées dans `try`/`on` silencieusement perdues

## ✅ Corrigé : capture par cellule verrouillée partagée, corps ET gestionnaire

Signalé par un cas réel : `examples/builtins/sdl.oc` chargeait bien la texture/la police
dans un `try` (`textureId = win.loadTexture(...)`), mais le logo et le texte
ne s'affichaient jamais — bien que le chargement ait réellement réussi.

### Cause

Le modèle "callback" de `try`/`on` (voir la doc de `lower_try`,
`src/lower/stmt.d/statements.d/exceptions.rs`) lowere le corps et le(s)
gestionnaire(s) en fonctions IR **séparées** de la fonction englobante
(`__try_body_N`/`__try_handler_N`, appelées via `__ocara_try_exec[_with_captures]`).
Une variable du scope extérieur référencée à l'intérieur était bien "capturée"
et transmise, mais par une **simple copie de valeur, à sens unique** : chargée
une fois dans un tableau (`__alloc_obj`), puis stockée dans un nouveau local
propre à la fonction séparée — aucun lien n'était conservé avec la variable
d'origine. Toute affectation à l'intérieur du `try`/`on` était donc perdue dès
la fin du bloc, silencieusement (pas d'erreur, pas de warning).

Cette capture-par-valeur ignorait le mécanisme déjà en place et déjà testé
pour les closures (`Expr::Nameless`, voir
[[memoire-concurrence-threads]]) : une capture y est promue en cellule
verrouillée sur le tas (`__alloc_locked_cell`), et le scope extérieur ET la
closure redirigent leurs accès vers ce même pointeur
(`LowerBuilder::heap_promoted` + `load_local`/`store_local`,
`src/lower/builder.d/types.rs`). `lower_try` n'avait simplement jamais été
mis à jour pour utiliser ce mécanisme.

Repro minimale (corps) : `var x:int = -1; try { x = 5 } on e {}; IO::writeln(x)`
affichait `-1` au lieu de `5`.

Repro minimale (gestionnaire, découverte en second — le gestionnaire n'avait
**aucun** mécanisme de capture du tout, même pas la copie à sens unique du
corps) : `var caught:bool = false; try { raise "boom" } on e { caught = true };
IO::writeln(caught)` affichait `false` au lieu de `true`. Confirmé aussi par
`examples/tests/20_try_failTest.oc`, un test préexistant qui reposait sur ce
même pattern : exécuté isolément, seule 1 assertion (celle faite *à l'intérieur*
du premier handler) passait sur les 6 attendues — la suite du test method
n'était jamais atteinte après l'échec silencieux de `assertTrue(caught)`.

### Correctif

`src/lower/stmt.d/statements.d/exceptions.rs` (`lower_try`) :

- Les captures du corps et de **tous** les gestionnaires sont maintenant
  collectées et fusionnées en une seule liste dédupliquée (le binding du
  handler, ex. `e`, est exclu via `param_names` pour ne pas se faire passer
  pour une capture s'il masque un nom du scope englobant).
- Côté appelant, la construction du tableau de captures réutilise exactement
  la logique déjà en place pour les closures : `__alloc_locked_cell` +
  `__locked_cell_set` pour une variable stack ordinaire, réutilisation directe
  du pointeur si déjà `heap_promoted` (par ce même try ou une closure
  englobante), ou lecture via `GetField` si la variable est elle-même une
  capture d'une closure englobante.
- Le corps (`__try_body_N`) et le gestionnaire (`__try_handler_N`) reçoivent
  chacun un pointeur vers ce même tableau (`__captures_ptr`, en plus de
  `err_val`/`err_type` pour le gestionnaire) et rechargent chaque entrée comme
  un pointeur de cellule verrouillée déjà promu (`heap_promoted`), exactement
  comme le fait une closure — pas de nouvelle Alloca stack.
- `runtime/src/lib.rs` (`__ocara_try_exec_with_captures`) transmet maintenant
  ce même pointeur de captures au gestionnaire (`handler(ev, et, captures_ptr)`),
  qui ne recevait auparavant que `(ev, et)`. `__ocara_try_exec` (sans captures)
  est inchangée : elle n'est utilisée que quand ni le corps ni aucun
  gestionnaire ne capture rien.

Vérifié : les deux reproductions minimales affichent maintenant la valeur
attendue ; `examples/tests/20_try_failTest.oc` passe ses 6 assertions (au lieu
de 1) ; `examples/builtins/sdl.oc` affiche maintenant réellement le logo et le
texte (confirmé par capture d'écran X11) ; `make regression` sans régression.

## Fichiers clés

`src/lower/stmt.d/statements.d/exceptions.rs` (`lower_try`), `runtime/src/lib.rs`
(`__ocara_try_exec_with_captures`), `src/lower/expr.d/lower.rs` (mécanisme de
promotion des captures de closure, réutilisé tel quel), `src/lower/builder.d/types.rs`
(`heap_promoted`, `load_local`/`store_local`), `examples/tests/20_try_failTest.oc`.
