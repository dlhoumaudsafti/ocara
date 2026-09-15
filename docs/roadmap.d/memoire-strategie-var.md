# Stratégie de gestion mémoire pour `var` (le mot-clé par défaut)

## ✅ Corrigé — analyse d'échappement statique, réutilisant le mécanisme `scoped`

## Stratégie retenue

**Analyse d'échappement statique** (pas de comptage de références, pas de GC) : un `var` est libéré automatiquement en fin de bloc — exactement comme un `scoped` implicite, même mécanisme de destruction déjà testé — quand le compilateur peut PROUVER, à la compilation, qu'il ne s'échappe jamais de sa fonction. Sinon (dès le moindre doute), il reste alloué jusqu'à la fin du programme, exactement comme avant ce chantier. Zéro coût à l'exécution : tout est décidé statiquement.

**Pourquoi pas le comptage de références** (l'alternative sérieuse envisagée) : ça couvrirait plus de cas (y compris l'échappement), mais (1) ne résout même pas complètement le problème — les structures auto-référentes déjà présentes dans la base de code (`Node<T>` en liste chaînée) créeraient des cycles qui fuient quand même sans un vrai GC de cycles, que ce projet exclut ; (2) coût permanent à l'exécution (incréments/décréments sur CHAQUE affectation/copie, et atomiques puisque ce langage a de vrais threads partageant des variables capturées) ; (3) touche une surface bien plus large du runtime/codegen (chaque `Store`, chaque retour de fonction, chaque insertion dans un tableau/map). L'analyse statique est à la fois plus fiable (preuve à la compilation, jamais d'état runtime qui pourrait se tromper) et plus optimisée (coût zéro).

## Ce qui a été construit

Un seul mécanisme (`src/sema/escape.rs`, nouveau module) sert DEUX besoins liés :

1. **Diagnostic E26 (`ArgumentEscape`)** — corrige au passage le bug documenté dans [memoire-echappement-argument.md](memoire-echappement-argument.md) : une `scoped`/`consumed` (ou, désormais, un `var`) passée en argument d'un constructeur/méthode **utilisateur** connue qui la stocke au-delà de l'appel (ex. `init(a) { self.data = a }`) est maintenant détectée et rejetée à la compilation, au lieu de corrompre silencieusement la mémoire.
2. **Libération automatique d'un `var`** — un `var` de type `string`/`array`/`map`/instance de classe utilisateur (jamais `Mutex`/`SQLite`/`MySQL`/`MariaDB`/`Thread` : fermer implicitement une ressource serait un changement de comportement bien plus surprenant) est enregistré dans le même mécanisme que `scoped` (`register_owned_local`/`emit_scope_drops`, `src/lower/stmt.d/ownership.rs`) quand il remplit DEUX conditions :
   - **Ne s'échappe jamais** (`escape::var_never_escapes`) : jamais retourné, jamais affecté à un champ/élément de tableau/map/autre variable, jamais capturé par une closure/thread, jamais passé à un paramètre prouvé retenu par un appel utilisateur connu, et — prudence spécifique à la libération auto, PAS au diagnostic E26 (voir "Asymétrie" ci-dessous) — jamais passé en argument à un appel dont le callee n'est pas résolu (tout builtin, ex. `Array::push(arr, x)`, qui retient bel et bien son argument).
   - **Initialisé par une allocation prouvée fraîche** (`ownership::is_fresh_allocation`) : `use Classe(...)`, un littéral tableau/map, une concaténation `+`, ou un littéral simple — jamais le résultat d'un appel quelconque (`.get()`, `Convert::*`, accès de champ/indexé...), qui pourrait retourner un ALIAS d'une valeur déjà possédée ailleurs plutôt qu'une valeur neuve.

### Asymétrie E26 vs libération auto (implémentée, pas seulement documentée)

Un appel dont le callee n'est PAS résolu (builtin, ou receveur dont le type n'est pas suivi) est traité différemment selon le consommateur :
- Pour E26 : jamais vérifié (rater un échappement n'est pas pire qu'aujourd'hui, où rien n'était vérifié du tout sur un argument).
- Pour la libération auto d'un `var` : **toujours traité comme retenant tous ses arguments** (`check_call_args(..., strict: bool)`, `src/sema/escape.rs`).

Cette distinction n'était initialement qu'une intention de conception — un **double-free confirmé par reproduction** (`acc.push(h)` où `h` retient un `var` passé en argument, puis relu via `acc.get(j)`) a forcé à l'implémenter réellement avant de considérer le chantier terminé.

### Second bug confirmé par reproduction : aliasing via accesseur

Un `var` dont l'initialiseur est un ACCESSEUR retournant une référence déjà possédée ailleurs (`var got:Holder = acc.get(j)`) ne doit **jamais** être libéré comme s'il possédait cette valeur uniquement — sinon double free dès que le conteneur (`acc`) la libère aussi. D'où la seconde condition ci-dessus (`is_fresh_allocation`) : sans elle, un `var` initialisé par n'importe quel appel (y compris un simple `.get()`) aurait pu être libéré à tort. Confirmé par reproduction (500 `Holder` poussés dans un tableau, relus via `.get()` dans une variable locale — SEGFAULT/double free sans ce garde-fou, plus aucun problème avec).

## Vérifié

- Repro du bug historique (`Box(arr)`) : rejeté à la compilation (E26).
- `Array::push(arr, x)` sur une `scoped`/`var` : toujours autorisé (pas de régression sur ce sucre déjà établi).
- Un `var` non-échappant utilisé/réaffecté des milliers de fois dans une boucle : mémoire réellement récupérée (peak RSS observé : ~2 Mo pour 3 millions d'itérations d'une string concaténée, contre plusieurs centaines de Mo si non libérée).
- Un `var` retenu par un constructeur puis stocké dans un tableau, relu ensuite (500 à 50 000 éléments) : aucune corruption, aucun crash.
- Un `var` capturé par une closure appelée plusieurs fois : jamais libéré prématurément.
- Réaffectation répétée d'un `var` non-échappant dans une boucle : ancienne valeur libérée avant chaque nouvelle affectation, valeur finale correcte.
- `make regression` sans régression sur l'intégralité de la base de code existante (49 + 408 PASS avant ce chantier, inchangé).
- `examples/tests/33_exception_hierarchyTest.oc` (E26 exercé indirectement via ses classes), `examples/tests/35_var_auto_freeTest.oc` (les 4 scénarios ci-dessus), tous verts.

## Limites assumées (imprécision, jamais dangereuses)

Toutes les approximations de l'analyse biaisent vers "ne libère PAS" (jamais l'inverse) — un `var` qui échapperait à travers une forme non modélisée ici reste, au pire, aussi peu optimisé qu'avant ce chantier, jamais corrompu :

- Un appel vers un récepteur dont le type n'est pas une classe utilisateur directement suivie (seul `self.methode()` est résolu pour un appel simple ; `StaticCall`/`New` résolvent n'importe quelle classe utilisateur par leur nom explicite) n'est jamais vérifié — traité prudemment comme non prouvé sûr.
- La propagation de taint à travers un `match` ne couvre que ses branches directes ; toute autre forme d'expression composée non modélisée explicitement (voir la doc de `src/sema/escape.rs`) est considérée fraîche/non tainted par défaut pour le CALCUL du taint (mais un `var` qui ne remplit pas la condition `is_fresh_allocation` est de toute façon exclu en amont).
- Les blocs runtime (`init`/`main`/`error`/`success`/`exit`) sont délibérément exclus de la libération automatique (`builder.auto_freeable_vars` n'y est jamais peuplé) : un `var` déclaré dans `init`/`main` peut rester visible dans un bloc suivant (`exit`) que cette analyse, bloc-par-bloc, ne voit pas — plutôt que risquer un faux négatif inter-blocs, ces blocs se comportent exactement comme avant ce chantier.
- Passer un `var` à `IO::writeln(v)` (identifiant nu) l'exclut de la libération auto (builtin non résolu, mode strict) — alors que l'interpoler dans un template (`` `${v}` ``) ne l'exclut pas (le template n'est jamais un identifiant nu, donc jamais tainté par la vérification d'argument). Cas réel le plus courant ("juste imprimer une variable") donc largement couvert malgré cette limite.

## Découverte annexe, non corrigée (hors périmètre) : `UnitTest::assertEquals` compare des pointeurs bruts, pas le contenu

En écrivant les tests de ce chantier, `UnitTest::assertEquals("valeur-boucle-1999", s)` (avec `s` une string construite dynamiquement par concaténation, contenu identique au littéral) échouait alors que `s` contenait bien la bonne valeur. `UnitTest_assertEquals` (`runtime/src/lib.rs`) compare littéralement `expected == actual` (deux `i64` bruts) — ça ne "marche" pour des strings que par coïncidence, quand les deux côtés partagent le même pointeur (deux littéraux identiques internés à la même adresse, ou la même valeur capturée). Les tests de ce chantier contournent le problème en comparant via l'opérateur `equal` du langage (comparaison de contenu réelle pour les strings, voir docs/EBNF.md) puis `assertTrue`/`assertFalse` sur le bool résultant. Non corrigé ici : distinct de la gestion mémoire de `var`, mériterait son propre chantier (`UnitTest::assertEquals` devrait dispatcher sur le tag runtime comme le fait déjà `__dyn_add`/les comparaisons strictes du langage).

## Fichiers clés

`src/sema/escape.rs` (nouveau — `compute_escaping_params`, `resolve_user_callable`, `trace_escapes_in_body`, `var_never_escapes`), `src/sema/typecheck.rs` (`check_argument_escape`, câblage E26), `src/sema/error.rs` (`ArgumentEscape`), `src/ir/module.rs` (`IrModule::escaping_params`/`class_members`), `src/lower/builder.d/program.rs` (peuplement), `src/lower/builder.d/functions.rs` (`compute_auto_freeable_vars` par fonction/méthode), `src/lower/stmt.d/ownership.rs` (`register_owned_local`, `emit_scope_drops`, `compute_auto_freeable_vars`, `is_fresh_allocation`), `examples/tests/33_exception_hierarchyTest.oc` (réutilisé pour E26, chantier précédent), `examples/tests/35_var_auto_freeTest.oc`.
