# Doubles libérations, use-after-free et fuites autour de `scoped`/`consumed`

## Doubles fermetures / doubles libérations

- Un `close()`/`destroy()` explicite sur une ressource (`SQLite`, `MySQL`, `Mutex`) **et** une variable `scoped` du même type déclenchent chacun leur propre libération — rien ne marque le slot comme déjà libéré après l'appel manuel (src/lower/stmt.d/ownership.rs:132-137, runtime/src/sqlite.rs:266-276, runtime/src/mutex.rs:144-152). Double-free.
- `Thread.join()`/`.detach()` appelé deux fois : même cause, le slot n'est jamais invalidé après le premier appel (runtime/src/thread.rs:129,154,160-163). Un appel après → use-after-free.
- `HTTPRequest`/`HTTPResponse` : type absent du système d'ownership (`ownership_class`), donc jamais géré par `scoped` — fuite garantie si `close()`/`closeResponse()` n'est pas appelé manuellement ; un double `close()` ou un mauvais cast entre les deux types → double-free/corruption (runtime/src/httprequest.rs:47-60,293-303).

## Bugs internes au mécanisme `scoped`/`consumed`

- **Drop dépendant du chemin d'exécution** : `OwnedLocalInfo.dropped` est un booléen par variable, pas par chemin (src/lower/stmt.d/ownership.rs:88). Un `return`/`break`/`continue` anticipé sur un chemin marque `dropped=true` pour tous les chemins → les autres sautent la libération. Fuite.
- **`consumed` en boucle → double free** : la sema compte les usages lexicalement (une seule occurrence textuelle = ok, src/sema/scope.rs:185-199), mais le drop est émis à chaque itération de la boucle (src/lower/stmt.d/block.rs:20) → le même pointeur est libéré plusieurs fois.
- **Réaffectation d'une `scoped`/`consumed`** (`s = nouvelleValeur`) : l'ancienne valeur du slot n'est jamais libérée avant d'être écrasée (src/lower/stmt.d/statements.d/assignments.rs:12-29) → fuite à chaque réaffectation, alors que l'EBNF présente explicitement `scoped` comme réaffectable (docs/EBNF.md:1176-1180).
- **Shadowing** : `owned_locals`/`block_scope_stack` sont des structures plates (src/lower/builder.d/types.rs:83-87) — un nom réutilisé dans un bloc frère écrase l'entrée précédente, perdant la trace d'une `scoped` externe encore vivante. Fuite.
- **`Stmt::Result`/`Stmt::Raise`** n'émettent aucun drop anticipé (contrairement à `Stmt::Return`) → fuite.
- Diagnostic `SemaError::OwnershipNotSupported` (src/sema/error.rs:42) déclaré mais jamais réellement émis — `scoped x:int` ou `scoped f:SDL` sont acceptés sans avertissement, avec une fausse impression de possession.

## Ampleur

Chaque point pris isolément est un correctif petit-à-moyen (suivre l'état "libéré" par variable ET par chemin d'exécution réel, invalider le slot après un appel manuel de fermeture). Pris ensemble, ça justifie une passe de durcissement dédiée du mécanisme `ownership.rs`/`class_ownership.rs` plutôt que des correctifs isolés au fil de l'eau.

## Fichiers clés

`src/lower/stmt.d/ownership.rs`, `src/lower/stmt.d/block.rs`, `src/lower/stmt.d/statements.d/assignments.rs`, `src/lower/builder.d/types.rs`, `src/sema/scope.rs`, `src/sema/error.rs`.
