# Mémoire partagée non synchronisée entre threads et closures

## Constat

Une variable capturée par une closure est "promue sur le tas" (`__alloc_obj(8)`, src/lower/expr.d/lower.rs:1340-1372) : le scope extérieur et la closure pointent vers la **même** cellule mémoire, lue/écrite sans aucun verrou. C'est reconnu dans le code lui-même comme un comportement non défini (runtime/src/thread.rs:19-23, runtime/src/httpserver.rs:35-37).

## Où ça se manifeste

- N'importe quel thread lancé (`Thread.run(closure)`) qui capture une variable du scope appelant.
- Le serveur HTTP : les handlers tournent sur plusieurs workers (4 par défaut, runtime/src/httpserver.rs:99/111) pouvant exécuter la même closure capturée en parallèle.
- Côté Rust, plusieurs `&'static mut` sont fabriqués depuis le même entier brut (runtime/src/httpserver.rs:142-155,368,394) — aliasing mutable simultané, UB au sens strict de Rust même si non observé en pratique avec le codegen actuel.
- `check_escape` (voir [memoire-echappement-argument](memoire-echappement-argument.md)) n'est jamais invoqué sur une variable capturée par une closure/thread — aucune vérification sema ne peut aujourd'hui prévenir ce cas.

## Ampleur

Deux directions possibles, toutes deux structurelles : (a) étendre l'analyse d'échappement aux captures pour au moins diagnostiquer le partage, (b) introduire un vrai mécanisme de synchronisation par défaut pour toute cellule capturée par plus d'un thread. Aucune des deux n'est un correctif localisé.

## Fichiers clés

`src/lower/expr.d/lower.rs`, `runtime/src/thread.rs`, `runtime/src/httpserver.rs`.
