# Mémoire partagée non synchronisée entre threads et closures

## ✅ Corrigé : synchronisation runtime réelle

Une variable capturée par une closure est "promue sur le tas" (`src/lower/expr.d/lower.rs`) : le scope extérieur et la closure partagent alors le même pointeur, potentiellement lu/écrit depuis plusieurs threads à la fois (`Thread::run`, workers HTTPServer) — comportement non défini avant ce correctif (reconnu comme tel dans le code lui-même, `runtime/src/thread.rs`/`httpserver.rs`).

**Corrigé** (option choisie : vraie synchronisation runtime, plutôt que rejeter le partage à la compilation) :
- `runtime/src/lib.rs` : nouvelle cellule verrouillée `__alloc_locked_cell` (mutex pthread + valeur i64, même convention "header avant le pointeur retourné" que les tags `TAG_*` existants), avec `__locked_cell_get`/`__locked_cell_set` qui verrouillent/déverrouillent à chaque accès.
- La promotion d'une capture au tas (`src/lower/expr.d/lower.rs`) utilise désormais `__alloc_locked_cell` au lieu du simple `__alloc_obj(8)` d'avant.
- `LowerBuilder::load_local`/`store_local` (`src/lower/builder.d/types.rs`), le point d'accès CENTRAL pour toute lecture/écriture de variable (utilisé aussi bien depuis le scope extérieur que depuis l'intérieur d'une closure via l'env), passent désormais par `__locked_cell_get`/`__locked_cell_set` pour toute variable `heap_promoted`/capturée, au lieu d'un `Load`/`Store` brut.

Vérifié : aucun changement nécessaire côté génération de code pour les types `float`/`bool` (le mécanisme d'appel Cranelift existant gère déjà le bitcast I64↔F64 selon la signature réelle du builtin appelé, indépendamment du type "logique" demandé dans l'IR — voir `src/codegen/emit.d/instructions.d/calls.rs`). `make regression` sans régression (y compris `thread`, `httpserver`, `httpserver_static`, `advanced_httpserver`).

## ⚠️ Limite assumée, choisie explicitement : pas d'atomicité des opérations composées

Un test réel (4 threads incrémentant un compteur capturé 5000 fois chacun, `counter = counter + 1`) confirme que le compteur final est **inférieur** à 20000 (pertes de mise à jour) malgré le correctif. **Ce n'est pas un bug** : `load_local`/`store_local` verrouillent chacun leur propre accès, mais la lecture et l'écriture d'une opération composée (`x = x + 1`) restent deux verrous séparés — la fenêtre entre les deux reste une vraie course, exactement comme un `int` normal en C ou un `volatile` en Java sans type atomique dédié. Ce que corrige ce chantier : l'absence totale de synchronisation (UB, torn reads/writes possibles, `&'static mut` aliasés simultanément côté Rust) — pas l'atomicité des opérations composées, qui demanderait un mécanisme plus invasif (verrouiller toute l'instruction d'affectation quand sa cible est une variable capturée, avec un risque de deadlock si l'expression référence une AUTRE variable capturée verrouillée séparément). **Décision prise avec David : rester sur la version sûre-mais-pas-atomique**, ne pas complexifier plus loin.

## Fichiers clés

`runtime/src/lib.rs` (`__alloc_locked_cell`/`__locked_cell_get`/`__locked_cell_set`), `src/lower/expr.d/lower.rs` (promotion des captures), `src/lower/builder.d/types.rs` (`load_local`/`store_local`), `src/codegen/desc.d/lowlevel.rs`.
